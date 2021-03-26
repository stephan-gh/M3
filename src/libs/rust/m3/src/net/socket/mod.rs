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
use crate::net::dataqueue::DataQueue;
use crate::net::{event, IpAddr, NetEvent, Port, Sd, SocketType};
use crate::rc::Rc;
use crate::session::NetworkManager;

mod raw;
mod tcp;
mod udp;

pub use self::raw::RawSocket;
pub use self::tcp::{StreamSocketArgs, TcpSocket};
pub use self::udp::{DgramSocketArgs, UdpSocket};

pub(crate) struct SocketArgs {
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
    Closing,
    /// The socket is closed (default state)
    Closed,
}

/// Socket prototype that is shared between sockets.
pub(crate) struct Socket {
    sd: Sd,
    ty: SocketType,

    state: Cell<State>,
    blocking: Cell<bool>,

    local_addr: Cell<IpAddr>,
    local_port: Cell<Port>,
    remote_addr: Cell<IpAddr>,
    remote_port: Cell<Port>,

    recv_queue: RefCell<DataQueue>,
}

impl Socket {
    pub fn new(sd: Sd, ty: SocketType) -> Rc<Self> {
        Rc::new(Self {
            sd,
            ty,

            state: Cell::new(State::Closed),
            blocking: Cell::new(true),

            local_addr: Cell::new(IpAddr::new(0, 0, 0, 0)),
            local_port: Cell::new(0),

            remote_addr: Cell::new(IpAddr::new(0, 0, 0, 0)),
            remote_port: Cell::new(0),

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

    pub fn set_local(&self, addr: IpAddr, port: Port, state: State) {
        self.local_addr.set(addr);
        self.local_port.set(port);
        self.state.set(state);
    }

    pub fn connect(
        &self,
        nm: &NetworkManager,
        remote_addr: IpAddr,
        remote_port: Port,
    ) -> Result<(), Error> {
        if self.state.get() == State::Connected {
            if !(self.remote_addr.get() == remote_addr && self.remote_port.get() == remote_port) {
                return Err(Error::new(Code::IsConnected));
            }
            return Ok(());
        }
        if self.state.get() == State::Closing {
            return Err(Error::new(Code::InvState));
        }

        if self.state.get() == State::Connecting {
            return Err(Error::new(Code::AlreadyInProgress));
        }

        let local_port = nm.connect(self.sd, remote_addr, remote_port)?;
        self.state.set(State::Connecting);
        self.remote_addr.set(remote_addr);
        self.remote_port.set(remote_port);
        self.local_port.set(local_port);

        if !self.blocking.get() {
            return Err(Error::new(Code::InProgress));
        }

        while self.state.get() == State::Connecting {
            nm.wait_for_events();
            nm.process_events(Some(self.sd));
        }

        if self.state.get() != State::Connected {
            Err(Error::new(Code::ConnectionFailed))
        }
        else {
            Ok(())
        }
    }

    pub fn accept(&self, nm: &NetworkManager) -> Result<(IpAddr, Port), Error> {
        if self.state.get() == State::Connected {
            return Ok((self.remote_addr.get(), self.remote_port.get()));
        }
        if self.state.get() == State::Connecting {
            return Err(Error::new(Code::AlreadyInProgress));
        }
        if self.state.get() != State::Listening {
            return Err(Error::new(Code::InvState));
        }

        self.state.set(State::Connecting);
        while self.state.get() == State::Connecting {
            nm.wait_for_events();
            nm.process_events(Some(self.sd));
        }

        if self.state.get() != State::Connected {
            Err(Error::new(Code::ConnectionFailed))
        }
        else {
            Ok((self.remote_addr.get(), self.remote_port.get()))
        }
    }

    pub fn has_data(&self) -> bool {
        self.recv_queue.borrow().has_data()
    }

    pub fn next_data<F, R>(
        &self,
        nm: &NetworkManager,
        amount: usize,
        mut consume: F,
    ) -> Result<R, Error>
    where
        F: FnMut(&[u8], IpAddr, Port) -> R,
    {
        loop {
            nm.process_events(Some(self.sd));

            if let Some(res) = self.recv_queue.borrow_mut().next_data(amount, &mut consume) {
                return Ok(res);
            }

            if !self.blocking.get() {
                nm.fetch_replies();
                return Err(Error::new(Code::WouldBlock));
            }

            nm.wait_for_events();
        }
    }

    pub fn send(
        &self,
        nm: &NetworkManager,
        data: &[u8],
        addr: IpAddr,
        port: Port,
    ) -> Result<(), Error> {
        loop {
            match nm.send(self.sd, addr, port, data) {
                Err(e) if e.code() != Code::NoCredits => break Err(e),
                Ok(_) => break Ok(()),
                _ => {},
            }

            if !self.blocking.get() {
                nm.fetch_replies();
                return Err(Error::new(Code::WouldBlock));
            }

            nm.wait_for_credits();

            nm.process_events(Some(self.sd));

            if self.state.get() == State::Closed {
                return Err(Error::new(Code::SocketClosed));
            }
        }
    }

    pub fn close(&self, nm: &NetworkManager) -> Result<(), Error> {
        let mut sent_req = false;

        while self.state.get() != State::Closed {
            if !sent_req {
                match nm.close(self.sd) {
                    Err(e) if e.code() == Code::NoCredits => {},
                    Err(e) => return Err(e),
                    Ok(_) => sent_req = true,
                }
            }

            if !self.blocking.get() {
                // TODO error inprogress?
                return Err(Error::new(Code::WouldBlock));
            }

            nm.wait_for_events();

            nm.process_events(Some(self.sd));
        }
        Ok(())
    }

    pub fn abort(&self, nm: &NetworkManager, remove: bool) -> Result<(), Error> {
        nm.abort(self.sd, remove)?;
        self.recv_queue.borrow_mut().clear();
        self.state.set(State::Closed);
        Ok(())
    }

    pub fn process_data_transfer(&self, event: NetEvent) {
        if self.ty != SocketType::Stream || self.state.get() != State::Closed {
            self.recv_queue.borrow_mut().append(event);
        }
    }

    pub fn process_connected(&self, msg: &event::ConnectedMessage) {
        self.state.set(State::Connected);
        self.remote_addr.set(IpAddr(msg.remote_addr as u32));
        self.remote_port.set(msg.remote_port as Port);
    }

    pub fn process_closed(&self, _msg: &event::ClosedMessage) {
        self.state.set(State::Closed);
    }

    pub fn process_close_req(&self, _msg: &event::CloseReqMessage) {
        self.state.set(State::Closing);
    }
}
