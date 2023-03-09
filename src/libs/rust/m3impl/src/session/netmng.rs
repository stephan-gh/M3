/*
 * Copyright (C) 2021-2022 Nils Asmussen, Barkhausen Institut
 * Copyright (C) 2021, Tendsin Mende <tendsin.mende@mailbox.tu-dresden.de>
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

use crate::com::{RecvGate, SendGate};
use crate::errors::Error;
use crate::net::{BaseSocket, Endpoint, IpAddr, NetEventChannel, Port, Sd, SocketArgs, SocketType};
use crate::rc::Rc;
use crate::session::ClientSession;
use crate::vfs::GenFileOp;

int_enum! {
    /// The operations for the network service
    pub struct NetworkOp : u64 {
        const STAT          = GenFileOp::STAT.val;
        const SEEK          = GenFileOp::SEEK.val;
        const NEXT_IN       = GenFileOp::NEXT_IN.val;
        const NEXT_OUT      = GenFileOp::NEXT_OUT.val;
        const COMMIT        = GenFileOp::COMMIT.val;
        const TRUNCATE      = GenFileOp::TRUNCATE.val;
        // TODO what about GenericFile::CLOSE?
        const BIND          = GenFileOp::REQ_NOTIFY.val + 1;
        const LISTEN        = GenFileOp::REQ_NOTIFY.val + 2;
        const CONNECT       = GenFileOp::REQ_NOTIFY.val + 3;
        const ABORT         = GenFileOp::REQ_NOTIFY.val + 4;
        const CREATE        = GenFileOp::REQ_NOTIFY.val + 5;
        const GET_IP        = GenFileOp::REQ_NOTIFY.val + 6;
        const GET_NAMESRV   = GenFileOp::REQ_NOTIFY.val + 7;
        const GET_SGATE     = GenFileOp::REQ_NOTIFY.val + 8;
        const OPEN_FILE     = GenFileOp::REQ_NOTIFY.val + 9;
    }
}

/// Represents a session at the network service, allowing to create and use sockets
///
/// To exchange events and data with the server, the [`NetEventChannel`] is used, which allows to
/// send and receive multiple messages. Events are used to receive connected or closed events from
/// the server and to send close requests to the server. Transmitted and received data is exchanged
/// via the [`NetEventChannel`] in both directions.
pub struct NetworkManager {
    client_session: ClientSession,
    metagate: SendGate,
}

impl NetworkManager {
    /// Creates a new instance for `service`
    pub fn new(service: &str) -> Result<Rc<Self>, Error> {
        let client_session = ClientSession::new(service)?;

        // Obtain meta gate for the service
        let sgate_crd =
            client_session.obtain(1, |sink| sink.push(NetworkOp::GET_SGATE), |_source| Ok(()))?;

        Ok(Rc::new(NetworkManager {
            client_session,
            metagate: SendGate::new_bind(sgate_crd.start()),
        }))
    }

    /// Returns the local IP address
    pub fn ip_addr(&self) -> Result<IpAddr, Error> {
        let mut reply = send_recv_res!(&self.metagate, RecvGate::def(), NetworkOp::GET_IP)?;
        let addr = IpAddr(reply.pop::<u32>()?);
        Ok(addr)
    }

    pub(crate) fn create(
        &self,
        ty: SocketType,
        protocol: Option<u8>,
        args: &SocketArgs,
    ) -> Result<BaseSocket, Error> {
        let mut sd = 0;
        let crd = self.client_session.obtain(
            2,
            |sink| {
                sink.push(NetworkOp::CREATE);
                sink.push(ty);
                sink.push(protocol.unwrap_or(0));
                sink.push(args.rbuf_size);
                sink.push(args.rbuf_slots);
                sink.push(args.sbuf_size);
                sink.push(args.sbuf_slots);
            },
            |source| {
                sd = source.pop()?;
                Ok(())
            },
        )?;

        let chan = NetEventChannel::new_client(crd.start())?;
        Ok(BaseSocket::new(sd, ty, chan))
    }

    pub(crate) fn nameserver(&self) -> Result<IpAddr, Error> {
        let mut reply = send_recv_res!(&self.metagate, RecvGate::def(), NetworkOp::GET_NAMESRV)?;
        let addr = IpAddr(reply.pop::<u32>()?);
        Ok(addr)
    }

    pub(crate) fn bind(&self, sd: Sd, port: Port) -> Result<(IpAddr, Port), Error> {
        let mut reply = send_recv_res!(&self.metagate, RecvGate::def(), NetworkOp::BIND, sd, port)?;
        let addr = IpAddr(reply.pop::<u32>()?);
        let port = reply.pop::<Port>()?;
        Ok((addr, port))
    }

    pub(crate) fn listen(&self, sd: Sd, port: Port) -> Result<IpAddr, Error> {
        let mut reply =
            send_recv_res!(&self.metagate, RecvGate::def(), NetworkOp::LISTEN, sd, port)?;
        let addr = IpAddr(reply.pop::<u32>()?);
        Ok(addr)
    }

    pub(crate) fn connect(&self, sd: Sd, endpoint: Endpoint) -> Result<Endpoint, Error> {
        let mut reply = send_recv_res!(
            &self.metagate,
            RecvGate::def(),
            NetworkOp::CONNECT,
            sd,
            endpoint.addr.0,
            endpoint.port
        )?;
        let addr = reply.pop::<u32>()?;
        let port = reply.pop::<Port>()?;
        Ok(Endpoint::new(IpAddr(addr), port))
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
