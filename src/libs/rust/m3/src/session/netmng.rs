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
use bitflags::bitflags;

use crate::cell::RefCell;
use crate::col::Vec;
use crate::com::{RecvGate, SendGate};
use crate::errors::Error;
use crate::net::{IpAddr, NetEventChannel, Port, Sd, Socket, SocketArgs, SocketType};
use crate::pes::VPE;
use crate::rc::Rc;
use crate::session::ClientSession;
use crate::tcu::TCU;

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
        const CREATE        = 10;
        const GET_SGATE     = 11;
        const OPEN_FILE     = 12;
    }
}

bitflags! {
    /// A bitmask of directions for [`NetworkManager::wait`].
    pub struct NetworkDirection : usize {
        /// Data can be received or the socket state has changed
        const INPUT         = 1;
        /// Data can be sent
        const OUTPUT        = 2;
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
        let sgate_crd = client_session.obtain(
            1,
            |sink| sink.push_word(NetworkOp::GET_SGATE.val),
            |_source| Ok(()),
        )?;

        Ok(NetworkManager {
            client_session,
            metagate: SendGate::new_bind(sgate_crd.start()),
            sockets: RefCell::new(Vec::new()),
        })
    }

    /// Waits until any socket has received input (including state-change events) or can produce
    /// output.
    ///
    /// Note that [`NetworkDirection::INPUT`] has to be specified to process events (state changes
    /// and data).
    ///
    /// Note also that this function uses [`VPE::sleep`] if no input/output on any socket is
    /// possible, which suspends the core until the next TCU message arrives. Thus, calling this
    /// function can only be done if all work is done.
    pub fn wait(&self, dirs: NetworkDirection) {
        loop {
            if self.tick_sockets(dirs) {
                break;
            }

            // ignore errors
            VPE::sleep().ok();
        }
    }

    /// Waits until any socket has received input (including state-change events) or can produce
    /// output or the given timeout in nanoseconds is reached.
    ///
    /// Note that [`NetworkDirection::INPUT`] has to be specified to process events (state changes
    /// and data).
    ///
    /// Note also that this function uses [`VPE::sleep`] if no input/output on any socket is
    /// possible, which suspends the core until the next TCU message arrives. Thus, calling this
    /// function can only be done if all work is done.
    pub fn wait_for(&self, timeout: u64, dirs: NetworkDirection) {
        let end = TCU::nanotime() + timeout;
        loop {
            let now = TCU::nanotime();
            if now >= end || self.tick_sockets(dirs) {
                break;
            }

            // ignore errors
            VPE::sleep_for(end - now).ok();
        }
    }

    fn tick_sockets(&self, dirs: NetworkDirection) -> bool {
        let mut found = false;
        for sock in self.sockets.borrow_mut().iter() {
            sock.fetch_replies();
            if (dirs.contains(NetworkDirection::INPUT) && sock.process_events())
                || (dirs.contains(NetworkDirection::OUTPUT) && sock.can_send())
            {
                found = true;
            }
        }
        found
    }

    pub(crate) fn create(
        &self,
        ty: SocketType,
        protocol: Option<u8>,
        args: &SocketArgs,
    ) -> Result<Rc<Socket>, Error> {
        let mut sd = 0;
        let crd = self.client_session.obtain(
            2,
            |sink| {
                sink.push_word(NetworkOp::CREATE.val);
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
}
