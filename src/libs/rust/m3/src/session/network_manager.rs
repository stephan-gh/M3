/*
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

use base::{int_enum, llog};

use crate::col::Treap;
use crate::col::Vec;
use crate::com::{MemGate, RecvGate, SendGate};
use crate::errors::{Code, Error};
use crate::net::{IpAddr, NetChannel, NetData, SocketType};
use crate::session::ClientSession;
use core::cell::RefCell;

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
        const CLOSE         = 6;
        const CREATE        = 7;
        const BIND          = 8;
        const LISTEN        = 9;
        const CONNECT       = 10;
        const ACCEPT        = 11;
        const COUNT         = 12;
        const QUERY_STATE   = 13;
        const TICK          = 14;
    }
}

pub struct NetworkManager {
    #[allow(dead_code)] // Needs to keep the session alive
    client_session: ClientSession,
    metagate: SendGate,
    channel: NetChannel,
    /// receive queue that holds all received net data, keyed by the socket_descriptor the data belongs to.
    receive_queue: RefCell<Treap<i32, Vec<NetData>>>,
}

impl NetworkManager {
    /// Creates a new instance for `service`. Returns Err if there was no network service with name `service`.
    pub fn new(service: &str) -> Result<Self, Error> {
        let client_session = ClientSession::new(service)?;
        // Obtain meta gate for the service
        let metagate = SendGate::new_bind(client_session.obtain_crd(1)?.start());
        let channel = NetChannel::bind(client_session.obtain_crd(3)?.start())?;
        Ok(NetworkManager {
            client_session,
            metagate,
            channel,
            receive_queue: RefCell::new(Treap::new()),
        })
    }

    pub fn create(&self, ty: SocketType, protocol: Option<u8>) -> Result<i32, Error> {
        let mut reply = send_recv_res!(
            &self.metagate,
            RecvGate::def(),
            NetworkOp::CREATE,
            ty as usize,
            protocol.unwrap_or(0)
        )?;
        let sd = reply.pop::<i32>()?;

        Ok(sd)
    }

    pub fn bind(&self, sd: i32, addr: IpAddr, port: u16) -> Result<(), Error> {
        let _reply = send_recv_res!(
            &self.metagate,
            RecvGate::def(),
            NetworkOp::BIND,
            sd,
            addr.0,
            port
        )?;
        Ok(())
    }

    pub fn listen(&self, sd: i32, local_addr: IpAddr, local_port: u16) -> Result<(), Error> {
        let _reply = send_recv_res!(
            &self.metagate,
            RecvGate::def(),
            NetworkOp::LISTEN,
            sd,
            local_addr.0,
            local_port
        )?;
        Ok(())
    }

    pub fn connect(
        &self,
        sd: i32,
        remote_addr: IpAddr,
        remote_port: u16,
        local_addr: IpAddr,
        local_port: u16,
    ) -> Result<(), Error> {
        llog!(
            DEF,
            "NetworkManager::connect(sd={}, remote_addr={:?}, remote_port={}, local_addr={:?}, local_port={})",
            sd,
            remote_addr,
            remote_port,
            local_addr,
            local_port
        );

        send_recv_res!(
            &self.metagate,
            RecvGate::def(),
            NetworkOp::CONNECT,
            sd,
            remote_addr.0,
            remote_port,
            local_addr.0,
            local_port
        )?;
        Ok(())
    }

    pub fn close(&self, sd: i32) -> Result<(), Error> {
        let _reply = send_recv_res!(&self.metagate, RecvGate::def(), NetworkOp::CLOSE, sd)?;
        Ok(())
    }

    pub fn as_file(
        &self,
        _sd: i32,
        _mode: i32,
        _mem: &MemGate,
        _memsize: usize,
    ) -> Result<(), Error> {
        // TODO FD?
        // TODO support file session creation.
        llog!(DEF, "Asfile not implemented for network manager");
        Err(Error::new(Code::NotSup))
    }

    /// Notifies the network manager that the socket at `sd` was dropped.
    pub fn notify_drop(&self, sd: i32) -> Result<(), Error> {
        // Notifies the server that this socket was dropped
        let _res = send_recv_res!(&self.metagate, RecvGate::def(), NetworkOp::CLOSE, sd)?;
        Ok(())
    }

    pub fn send(
        &self,
        sd: i32,
        src_addr: IpAddr,
        src_port: u16,
        dst_addr: IpAddr,
        dst_port: u16,
        data: &[u8],
    ) -> Result<(), Error> {
        let wrapped_data = NetData::from_slice(sd, data, src_addr, src_port, dst_addr, dst_port);
        // Send data over channel to service

        let channel_res = self.channel.send(wrapped_data);

        // ticks the server
        send_recv_res!(&self.metagate, RecvGate::def(), NetworkOp::TICK)?;

        channel_res
    }

    /// Pulls all messages that are on the channel
    fn update_recv_queue(&self) -> Result<(), Error> {
        while let Ok(data) = self.channel.receive() {
            let mut queue_lock = self.receive_queue.borrow_mut();
            if let Some(sdqueue) = queue_lock.get_mut(&data.sd) {
                sdqueue.push(data);
            }
            else {
                let sd = data.sd;
                let mut dvec = Vec::with_capacity(1);
                dvec.push(data);
                queue_lock.insert(sd, dvec);
            }
        }

        Ok(())
    }

    /// Updates the recv queue for all sockets on this manager. The returns the first package for this socket
    /// if there is some.
    /// Return format is: ((source_ip, source_port), data)
    pub fn recv<'a>(&'a self, sd: i32) -> Result<NetData, Error> {
        // ticks the server

        send_recv_res!(&self.metagate, RecvGate::def(), NetworkOp::TICK)?;

        // after ticking, check recv queue
        self.update_recv_queue()?;
        if let Some(recv_queue) = self.receive_queue.borrow_mut().get_mut(&sd) {
            if recv_queue.len() > 0 {
                let package = recv_queue.remove(0);
                return Ok(package);
            }
        }

        Err(Error::new(Code::NotFound))
    }

    pub fn get_state(&self, sd: i32) -> Result<crate::net::SocketState, Error> {
        let mut res_stream =
            send_recv_res!(&self.metagate, RecvGate::def(), NetworkOp::QUERY_STATE, sd)?;
        res_stream.pop::<crate::net::SocketState>()
    }
}
