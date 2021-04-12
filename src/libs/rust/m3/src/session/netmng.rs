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

use crate::cell::RefCell;
use crate::col::Vec;
use crate::com::{RecvGate, SendGate};
use crate::errors::Error;
use crate::net::{IpAddr, NetEventChannel, Port, Sd, Socket, SocketArgs, SocketType};
use crate::pes::VPE;
use crate::rc::Rc;
use crate::session::ClientSession;

int_enum! {
    /// The operations for the network service
    pub struct NetworkOp : u64 {
        const STAT          = 0;
        const SEEK          = 1;
        #[allow(non_camel_case_types)]
        const NEXT_IN       = 2;
        #[allow(non_camel_case_types)]
        const NEXT_OUT      = 3;
        const COMMIT        = 4;
        // TODO what about GenericFile::CLOSE?
        const BIND          = 6;
        const LISTEN        = 7;
        const CONNECT       = 8;
        const ABORT         = 9;
    }
}

/// Represents a session at the network service, allowing to create and use sockets
///
/// To exchange events and data with the server, the [`NetEventChannel`] is used, which allows to
/// send and receive multiple messages. Events are used to receive connected or closed events from
/// the server and to send close requests to the server. Transmitted and received data is exchanged
/// via the [`NetEventChannel`] in both directions.
pub struct NetworkManager {
    #[allow(dead_code)] // Needs to keep the session alive
    client_session: ClientSession,
    metagate: SendGate,
    sockets: RefCell<Vec<Rc<Socket>>>,
}

impl NetworkManager {
    /// Creates a new instance for `service`
    pub fn new(service: &str) -> Result<Self, Error> {
        let client_session = ClientSession::new(service)?;
        // Obtain meta gate for the service
        let metagate = SendGate::new_bind(client_session.obtain_crd(1)?.start());
        Ok(NetworkManager {
            client_session,
            metagate,
            sockets: RefCell::new(Vec::new()),
        })
    }

    pub(crate) fn create(
        &self,
        ty: SocketType,
        protocol: Option<u8>,
        args: &SocketArgs,
    ) -> Result<Rc<Socket>, Error> {
        let mut sd = 0;
        let crd = self.client_session.obtain(
            3,
            |sink| {
                sink.push_word(ty as u64);
                sink.push_word(protocol.unwrap_or(0) as u64);
                sink.push_word(args.rbuf_size as u64);
                sink.push_word(args.rbuf_slots as u64);
                sink.push_word(args.sbuf_size as u64);
                sink.push_word(args.sbuf_slots as u64);
            },
            |source| {
                sd = source.pop_word()? as Sd;
                Ok(())
            },
        )?;

        let chan = NetEventChannel::new_client(crd.start())?;
        let sock = Socket::new(sd, ty, chan);
        self.sockets.borrow_mut().push(sock.clone());
        Ok(sock)
    }

    pub(crate) fn remove_socket(&self, sd: Sd) {
        self.sockets.borrow_mut().retain(|s| s.sd() != sd);
    }

    pub(crate) fn bind(&self, sd: Sd, port: Port) -> Result<IpAddr, Error> {
        let mut reply = send_recv_res!(&self.metagate, RecvGate::def(), NetworkOp::BIND, sd, port)?;
        let addr = IpAddr(reply.pop::<u32>()?);
        Ok(addr)
    }

    pub(crate) fn listen(&self, sd: Sd, port: Port) -> Result<IpAddr, Error> {
        let mut reply =
            send_recv_res!(&self.metagate, RecvGate::def(), NetworkOp::LISTEN, sd, port)?;
        let addr = IpAddr(reply.pop::<u32>()?);
        Ok(addr)
    }

    pub(crate) fn connect(
        &self,
        sd: Sd,
        remote_addr: IpAddr,
        remote_port: Port,
    ) -> Result<Port, Error> {
        let mut reply = send_recv_res!(
            &self.metagate,
            RecvGate::def(),
            NetworkOp::CONNECT,
            sd,
            remote_addr.0,
            remote_port
        )?;
        Ok(reply.pop::<Port>()?)
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

    /// Waits until any socket or a specific socket has received an event
    ///
    /// If `wait_sd` is None, the function waits until any socket has received an event. Otherwise,
    /// it waits until the socket with this socket descriptor has received an event.
    ///
    /// Note: this function uses [`VPE::sleep`] if no events are present, which suspends the core
    /// until the next TCU message arrives. Thus, calling this function can only be done if all work
    /// is done.
    pub fn wait_for_events(&self, wait_sd: Option<Sd>) {
        loop {
            for sock in self.sockets.borrow_mut().iter() {
                if sock.process_events() {
                    if let Some(sd) = wait_sd {
                        if sd != sock.sd() {
                            continue;
                        }
                    }
                    return;
                }
            }

            // ignore errors
            VPE::sleep().ok();
        }
    }

    /// Waits until any socket or a specific socket can send events to the server
    ///
    /// If `wait_sd` is None, the function waits until any socket can send. Otherwise, it waits
    /// until the socket with this socket descriptor can send.
    ///
    /// Note: this function uses [`VPE::sleep`] if no credits are available, which suspends the core
    /// until the next TCU message arrives. Thus, calling this function can only be done if all work
    /// is done.
    pub fn wait_for_credits(&self, wait_sd: Option<Sd>) {
        loop {
            for sock in self.sockets.borrow_mut().iter() {
                if sock.can_send() {
                    if let Some(sd) = wait_sd {
                        if sd != sock.sd() {
                            continue;
                        }
                    }
                    return;
                }
            }

            // ignore errors
            VPE::sleep().ok();
        }
    }
}
