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

use crate::errors::{Code, Error};
use crate::net::{
    socket::{Socket, State},
    IpAddr, Port, Sd, SocketType,
};
use crate::rc::Rc;
use crate::session::NetworkManager;

pub struct UdpSocket<'n> {
    socket: Rc<Socket>,
    nm: &'n NetworkManager,
}

impl<'n> UdpSocket<'n> {
    pub fn new(nm: &'n NetworkManager) -> Result<Self, Error> {
        Ok(UdpSocket {
            socket: nm.create(SocketType::Dgram, None)?,
            nm,
        })
    }

    pub fn sd(&self) -> Sd {
        self.socket.sd()
    }

    pub fn state(&self) -> State {
        self.socket.state()
    }

    pub fn blocking(&self) -> bool {
        self.socket.blocking()
    }

    pub fn set_blocking(&mut self, blocking: bool) {
        self.socket.set_blocking(blocking);
    }

    pub fn bind(&mut self, addr: IpAddr, port: Port) -> Result<(), Error> {
        if self.socket.state() != State::Closed {
            return Err(Error::new(Code::InvState));
        }

        self.nm.bind(self.socket.sd(), addr, port)?;
        self.socket.set_local(addr, port, State::Bound);
        Ok(())
    }

    pub fn has_data(&self) -> bool {
        self.socket.has_data()
    }

    pub fn recv(&self, data: &mut [u8]) -> Result<usize, Error> {
        self.recv_from(data).map(|(size, _, _)| size)
    }

    pub fn recv_from(&self, data: &mut [u8]) -> Result<(usize, IpAddr, Port), Error> {
        self.socket
            .next_data(self.nm, data.len(), |buf, addr, port| {
                data[0..buf.len()].copy_from_slice(buf);
                (buf.len(), addr, port)
            })
    }

    pub fn send_to(&self, data: &[u8], addr: IpAddr, port: Port) -> Result<(), Error> {
        self.socket.send(self.nm, data, addr, port)
    }

    pub fn abort(&mut self) -> Result<(), Error> {
        self.socket.abort(self.nm)
    }
}

impl Drop for UdpSocket<'_> {
    fn drop(&mut self) {
        // ignore errors
        self.abort().ok();
    }
}
