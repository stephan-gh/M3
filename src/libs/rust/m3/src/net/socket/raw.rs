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

use crate::col::Vec;
use crate::errors::Error;
use crate::net::{socket::Socket, SocketType};
use crate::session::NetworkManager;

/// A Raw socket sends already finished packages. Therefore the IpHeader must be written, before the package is passed to send.
pub struct RawSocket<'a> {
    #[allow(dead_code)]
    socket: Socket<'a>,
}

impl<'a> RawSocket<'a> {
    pub fn new(network_manager: &'a NetworkManager, protocol: Option<u8>) -> Result<Self, Error> {
        Ok(RawSocket {
            socket: Socket::new(SocketType::Raw, network_manager, protocol)?,
        })
    }

    pub fn send(_data: &[u8]) -> Result<usize, Error> {
        Ok(0)
    }

    pub fn recv_msg<T>(&self) -> Result<Vec<T>, Error> {
        Ok(Vec::new())
    }
}
