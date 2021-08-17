/*
 * Copyright (C) 2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Copyright (C) 2021, Tendsin Mende <tendsin.mende@mailbox.tu-dresden.de>
 * Copyright (C) 2017, Georg Kotheimer <georg.kotheimer@mailbox.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * This file is part of M3 (Microkernel-based SysteM for Heterogeneous Manycores).
 *
 * M3 is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License version 2 as
 * published by the Free Software Foundation.
 *
 * M3 is distributed in the hope that it will be useful, but
 * WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
 * General Public License version 2 for more details.
 */

use crate::cell::{Cell, RefCell};
use crate::errors::{Code, Error};
use crate::llog;
use crate::net::dataqueue::DataQueue;
use crate::net::{
    event, Endpoint, IpAddr, NetEvent, NetEventChannel, NetEventType, Port, Sd, SocketType,
};
use crate::rc::Rc;

mod raw;
mod tcp;
mod udp;

pub use self::raw::RawSocket;
pub use self::tcp::{StreamSocketArgs, TcpSocket};
pub use self::udp::{DgramSocketArgs, UdpSocket};

const EVENT_FETCH_BATCH_SIZE: u32 = 4;

pub struct SocketArgs {
    pub rbuf_slots: usize,
    pub rbuf_size: usize,
    pub sbuf_slots: usize,
    pub sbuf_size: usize,
}

impl Default for SocketArgs {
    fn default() -> Self {
        Self {
            rbuf_slots: 4,
            rbuf_size: 16 * 1024,
            sbuf_slots: 4,
            sbuf_size: 16 * 1024,
        }
    }
}

/// The states sockets can be in
#[derive(Eq, Debug, PartialEq, Clone, Copy)]
pub enum State {
    /// The socket is bound to a local address and port
    Bound,
    /// The socket is listening on a local address and port for remote connections
    Listening,
    /// The socket is currently connecting to a remote endpoint
    Connecting,
    /// The socket is connected to a remote endpoint
    Connected,
    /// The remote side has closed the connection
    RemoteClosed,
    /// The socket is currently being closed, initiated by our side
    Closing,
    /// The socket is closed (default state)
    Closed,
}

/// Socket prototype that is shared between sockets.
pub(crate) struct Socket {
    sd: Sd,
    ty: SocketType,
    blocking: Cell<bool>,

    pub state: Cell<State>,

    pub local_ep: Cell<Option<Endpoint>>,
    pub remote_ep: Cell<Option<Endpoint>>,

    pub channel: Rc<NetEventChannel>,
    pub recv_queue: RefCell<DataQueue>,
}

impl Socket {
    pub fn new(sd: Sd, ty: SocketType, channel: Rc<NetEventChannel>) -> Rc<Self> {
        Rc::new(Self {
            sd,
            ty,

            state: Cell::new(State::Closed),
            blocking: Cell::new(true),

            local_ep: Cell::new(None),
            remote_ep: Cell::new(None),

            channel,
            recv_queue: RefCell::new(DataQueue::default()),
        })
    }

    pub fn sd(&self) -> Sd {
        self.sd
    }

    pub fn state(&self) -> State {
        self.state.get()
    }

    pub fn blocking(&self) -> bool {
        self.blocking.get()
    }

    pub fn set_blocking(&self, blocking: bool) {
        self.blocking.set(blocking);
    }

    pub fn disconnect(&self) {
        self.local_ep.set(None);
        self.remote_ep.set(None);
        self.state.set(State::Closed);
    }

    pub fn has_data(&self) -> bool {
        self.recv_queue.borrow().has_data()
    }

    pub fn has_all_credits(&self) -> bool {
        self.channel.has_all_credits()
    }

    pub fn next_data<F, R>(&self, amount: usize, mut consume: F) -> Result<R, Error>
    where
        F: FnMut(&[u8], Endpoint) -> (usize, R),
    {
        loop {
            if let Some(res) = self.recv_queue.borrow_mut().next_data(amount, &mut consume) {
                return Ok(res);
            }

            if !self.blocking.get() {
                self.process_events();
                return Err(Error::new(Code::WouldBlock));
            }

            self.wait_for_events();
        }
    }

    pub fn send(&self, data: &[u8], endpoint: Endpoint) -> Result<(), Error> {
        loop {
            let res = self.channel.send_data(endpoint, data.len(), |buf| {
                buf.copy_from_slice(data);
            });
            match res {
                Err(e) if e.code() != Code::NoCredits => break Err(e),
                Ok(_) => break Ok(()),
                _ => {},
            }

            if !self.blocking.get() {
                self.fetch_replies();
                return Err(Error::new(Code::WouldBlock));
            }

            self.wait_for_credits();

            if self.state.get() == State::Closed {
                return Err(Error::new(Code::SocketClosed));
            }
        }
    }

    pub fn fetch_replies(&self) {
        self.channel.fetch_replies();
    }

    pub fn can_send(&self) -> bool {
        self.channel.can_send().unwrap()
    }

    pub fn process_events(&self) -> bool {
        let mut res = false;
        for _ in 0..EVENT_FETCH_BATCH_SIZE {
            if let Some(event) = self.channel.receive_event() {
                self.process_event(event);
                res = true;
            }
            else {
                break;
            }
        }
        res
    }

    pub fn tear_down(&self) {
        // make sure that all packets we sent are seen and handled by the server. thus, wait until
        // we have got all replies to our potentially in-flight packets, in which case we also have
        // received our credits back.
        while !self.has_all_credits() {
            self.wait_for_credits();
        }
    }

    fn wait_for_events(&self) {
        while !self.process_events() {
            self.channel.wait_for_events();
        }
    }

    fn wait_for_credits(&self) {
        loop {
            self.fetch_replies();
            if self.can_send() {
                break;
            }
            self.channel.wait_for_credits();
        }
    }

    fn process_event(&self, event: NetEvent) {
        match event.msg_type() {
            NetEventType::DATA => {
                if self.ty != SocketType::Stream
                    || (self.state.get() != State::Closing && self.state.get() != State::Closed)
                {
                    let _msg = event.msg::<event::DataMessage>();
                    llog!(
                        NET,
                        "socket {}: received data with {}b from {}",
                        self.sd,
                        _msg.size,
                        Endpoint::new(IpAddr(_msg.addr as u32), _msg.port as Port)
                    );
                    self.recv_queue.borrow_mut().append(event, 0);
                }
            },

            NetEventType::CONNECTED => {
                let msg = event.msg::<event::ConnectedMessage>();
                let ep = Endpoint::new(IpAddr(msg.remote_addr as u32), msg.remote_port as Port);
                llog!(NET, "socket {}: connected to {}", self.sd, ep);
                self.state.set(State::Connected);
                self.remote_ep.set(Some(ep));
            },

            NetEventType::CLOSED => {
                llog!(NET, "socket {}: closed", self.sd);
                self.disconnect();
            },

            NetEventType::CLOSE_REQ => {
                llog!(NET, "socket {}: remote side was closed", self.sd);
                self.state.set(State::RemoteClosed);
            },

            t => panic!("unexpected message type {}", t),
        }
    }
}
