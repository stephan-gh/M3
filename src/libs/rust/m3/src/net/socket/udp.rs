/*
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

use crate::errors::{Code, Error};
use crate::net::{socket::Socket, IpAddr, NetData, SocketState, SocketType};
use crate::session::NetworkManager;

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum UdpState {
    ///If the socket is not bound to any address
    Unbound,
    ///If the socket was bound to some address
    Open,
    ///Some invalid state of the socket
    Invalid,
}

impl UdpState {
    pub fn from_u64(other: u64) -> UdpState {
        match other {
            0 => UdpState::Unbound,
            1 => UdpState::Open,
            _ => UdpState::Invalid,
        }
    }
}

pub struct UdpSocket<'a> {
    socket: Socket<'a>,
    is_blocking: bool,
}

impl<'a> UdpSocket<'a> {
    pub fn new(network_manager: &'a NetworkManager) -> Result<Self, Error> {
        Ok(UdpSocket {
            socket: Socket::new(SocketType::Dgram, network_manager, None)?,
            is_blocking: false,
        })
    }

    pub fn set_blocking(&mut self, should_block: bool) {
        self.is_blocking = should_block;
    }

    pub fn bind(&self, addr: IpAddr, port: u16) -> Result<(), Error> {
        self.socket.nm.bind(self.socket.sd, addr, port)
    }

    pub fn recv(&self) -> Result<NetData, Error> {
        if self.is_blocking {
            loop {
                let pkg = self.socket.nm.recv(self.socket.sd);
                if pkg.is_ok() {
                    return pkg;
                }
            }
        }
        else {
            self.socket.nm.recv(self.socket.sd)
        }
    }

    pub fn send(&self, dest_addr: IpAddr, dest_port: u16, data: &[u8]) -> Result<(), Error> {
        //Only specify destination address, source is handled by the server
        self.socket.nm.send(
            self.socket.sd,
            IpAddr::unspecified(),
            0,
            dest_addr,
            dest_port,
            data,
        )
    }

    ///Queries the socket state from the server. Can be used to wait for the socket to change into a specific state.
    pub fn state(&mut self) -> Result<UdpState, Error> {
        let state = self.socket.nm.get_state(self.socket.sd)?;
        if let SocketState::UdpState(st) = state {
            Ok(st)
        }
        else {
            println!("State was: {:?}", state);
            Err(Error::new(Code::WrongSocketType))
        }
    }
}
