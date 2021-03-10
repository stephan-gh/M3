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

use crate::errors::Error;
use crate::net::{IpAddr, SocketType};
use crate::session::NetworkManager;

mod raw;
mod tcp;
mod udp;

pub use self::raw::RawSocket;
pub use self::tcp::{TcpSocket, TcpState};
pub use self::udp::{UdpSocket, UdpState};

///Socket prototype that is shared between sockets.
pub(crate) struct Socket<'a> {
    pub sd: i32,

    pub local_addr: IpAddr,
    pub local_port: u16,
    pub remote_addr: IpAddr,
    pub remote_port: u16,

    pub nm: &'a NetworkManager,
}

impl<'a> Drop for Socket<'a> {
    fn drop(&mut self) {
        //Notify that we dropped, but don't care for the outcome. This just makes sure that the "CLOSE"
        //Is actually send to the server, even if the user didn't program it.
        let _ = self.nm.notify_drop(self.sd);
    }
}

impl<'a> Socket<'a> {
    pub fn new(
        ty: SocketType,
        network_manager: &'a NetworkManager,
        protocol: Option<u8>,
    ) -> Result<Self, Error> {
        //Allocate self on the network manager
        let sd = network_manager.create(ty, protocol)?;
        Ok(Self {
            sd,
            local_addr: IpAddr::new(0, 0, 0, 0),
            local_port: 0,

            remote_addr: IpAddr::new(0, 0, 0, 0),
            remote_port: 0,

            nm: network_manager,
        })
    }
}
