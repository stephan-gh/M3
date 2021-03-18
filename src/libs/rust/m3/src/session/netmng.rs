/*
 * Copyright (C) 2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Copyright (C) 2021, Tendsin Mende <tendsin.mende@mailbox.tu-dresden.de>
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

use base::int_enum;

use crate::cell::{Ref, RefCell};
use crate::col::Vec;
use crate::com::{RecvGate, SendGate};
use crate::errors::Error;
use crate::net::{
    event, IpAddr, NetEvent, NetEventChannel, NetEventType, Port, Sd, Socket, SocketType,
};
use crate::pes::VPE;
use crate::rc::Rc;
use crate::session::ClientSession;

const EVENT_FETCH_BATCH_SIZE: u32 = 4;

int_enum! {
    /// The operations for [`GenericFile`].
    pub struct NetworkOp : u64 {
        const STAT          = 0;
        const SEEK          = 1;
        #[allow(non_camel_case_types)]
        const NEXT_IN       = 2;
        #[allow(non_camel_case_types)]
        const NEXT_OUT      = 3;
        const COMMIT        = 4;
        // TODO what about GenericFile::CLOSE?
        const CREATE        = 6;
        const BIND          = 7;
        const LISTEN        = 8;
        const CONNECT       = 9;
        const ABORT         = 10;
    }
}

pub struct NetworkManager {
    #[allow(dead_code)] // Needs to keep the session alive
    client_session: ClientSession,
    metagate: SendGate,
    channel: Rc<NetEventChannel>,
    sockets: RefCell<Vec<Rc<Socket>>>,
}

impl NetworkManager {
    /// Creates a new instance for `service`. Returns Err if there was no network service with name `service`.
    pub fn new(service: &str) -> Result<Self, Error> {
        let client_session = ClientSession::new(service)?;
        // Obtain meta gate for the service
        let metagate = SendGate::new_bind(client_session.obtain_crd(1)?.start());
        let chan_caps = client_session.obtain_crd(3)?.start();
        let channel = NetEventChannel::new_client(chan_caps)?;
        Ok(NetworkManager {
            client_session,
            metagate,
            channel,
            sockets: RefCell::new(Vec::new()),
        })
    }

    pub(crate) fn create(&self, ty: SocketType, protocol: Option<u8>) -> Result<Rc<Socket>, Error> {
        let mut reply = send_recv_res!(
            &self.metagate,
            RecvGate::def(),
            NetworkOp::CREATE,
            ty as usize,
            protocol.unwrap_or(0)
        )?;

        let sd = reply.pop::<Sd>()?;
        let sock = Socket::new(sd, ty);
        self.sockets.borrow_mut().push(sock.clone());
        Ok(sock)
    }

    pub(crate) fn remove_socket(&self, sd: Sd) {
        self.sockets.borrow_mut().retain(|s| s.sd() != sd);
    }

    pub(crate) fn bind(&self, sd: Sd, addr: IpAddr, port: Port) -> Result<(), Error> {
        send_recv_res!(
            &self.metagate,
            RecvGate::def(),
            NetworkOp::BIND,
            sd,
            addr.0,
            port
        )
        .map(|_| ())
    }

    pub(crate) fn listen(&self, sd: Sd, addr: IpAddr, port: Port) -> Result<(), Error> {
        send_recv_res!(
            &self.metagate,
            RecvGate::def(),
            NetworkOp::LISTEN,
            sd,
            addr.0,
            port
        )
        .map(|_| ())
    }

    pub(crate) fn connect(
        &self,
        sd: Sd,
        remote_addr: IpAddr,
        remote_port: Port,
        local_port: Port,
    ) -> Result<(), Error> {
        send_recv_res!(
            &self.metagate,
            RecvGate::def(),
            NetworkOp::CONNECT,
            sd,
            remote_addr.0,
            remote_port,
            local_port
        )
        .map(|_| ())
    }

    pub(crate) fn close(&self, sd: Sd) -> Result<(), Error> {
        self.channel.send_close_req(sd)
    }

    pub(crate) fn abort(&self, sd: Sd, remove: bool) -> Result<(), Error> {
        send_recv_res!(
            &self.metagate,
            RecvGate::def(),
            NetworkOp::ABORT,
            sd,
            remove
        )
        .map(|_| ())
    }

    pub(crate) fn send(
        &self,
        sd: Sd,
        dst_addr: IpAddr,
        dst_port: Port,
        data: &[u8],
    ) -> Result<(), Error> {
        self.channel
            .send_data(sd, dst_addr, dst_port, data.len(), |buf| {
                buf.copy_from_slice(data);
            })
    }

    pub fn wait_sync(&self) {
        while !self.channel.has_events() {
            // ignore errors
            VPE::sleep().ok();

            self.channel.fetch_replies();
        }
    }

    pub fn process_events(&self, socket: Option<Sd>) {
        for _ in 0..EVENT_FETCH_BATCH_SIZE {
            if let Some(event) = self.channel.receive_event() {
                if let Some(sd) = self.process_event(event) {
                    if sd == socket.unwrap_or(Sd::MAX) {
                        break;
                    }
                }
            }
            else {
                break;
            }
        }
    }

    fn process_event(&self, event: NetEvent) -> Option<Sd> {
        let sockets = self.sockets.borrow();
        match event.msg_type() {
            NetEventType::DATA => {
                let sd = event.msg::<event::DataMessage>().sd as Sd;
                if let Some(sock) = Self::get_socket(&sockets, sd) {
                    sock.process_data_transfer(event);
                }
                Some(sd)
            },

            NetEventType::CONNECTED => {
                let msg = event.msg::<event::ConnectedMessage>();
                if let Some(sock) = Self::get_socket(&sockets, msg.sd as Sd) {
                    sock.process_connected(&msg);
                }
                Some(msg.sd as Sd)
            },

            NetEventType::CLOSED => {
                let msg = event.msg::<event::ClosedMessage>();
                if let Some(sock) = Self::get_socket(&sockets, msg.sd as Sd) {
                    sock.process_closed(&msg);
                }
                Some(msg.sd as Sd)
            },

            NetEventType::CLOSE_REQ => {
                let msg = event.msg::<event::CloseReqMessage>();
                if let Some(sock) = Self::get_socket(&sockets, msg.sd as Sd) {
                    sock.process_close_req(&msg);
                }
                Some(msg.sd as Sd)
            },

            t => panic!("unexpected message type {}", t),
        }
    }

    fn get_socket<'s>(sockets: &'s Ref<'_, Vec<Rc<Socket>>>, sd: Sd) -> Option<&'s Rc<Socket>> {
        for s in sockets.iter() {
            if s.sd() == sd {
                return Some(s);
            }
        }
        None
    }
}
