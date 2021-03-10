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

pub struct TcpSocket<'n> {
    socket: Rc<Socket>,
    nm: &'n NetworkManager,
}

impl<'n> TcpSocket<'n> {
    pub fn new(nm: &'n NetworkManager) -> Result<Self, Error> {
        Ok(TcpSocket {
            socket: nm.create(SocketType::Stream, None)?,
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

    pub fn listen(&mut self, addr: IpAddr, port: Port) -> Result<(), Error> {
        if self.socket.state() != State::Closed {
            return Err(Error::new(Code::InvState));
        }

        self.nm.listen(self.socket.sd(), addr, port)?;
        self.socket.set_local(addr, port, State::Listening);
        Ok(())
    }

    pub fn connect(
        &mut self,
        remote_addr: IpAddr,
        remote_port: Port,
        local_port: Port,
    ) -> Result<(), Error> {
        self.socket
            .connect(self.nm, remote_addr, remote_port, local_port)
    }

    pub fn accept(&mut self) -> Result<(IpAddr, Port), Error> {
        self.socket.accept(self.nm)
    }

    pub fn has_data(&self) -> bool {
        self.socket.has_data()
    }

    pub fn recv(&mut self, data: &mut [u8]) -> Result<usize, Error> {
        self.recv_from(data).map(|(size, _addr, _port)| size)
    }

    pub fn recv_from(&mut self, data: &mut [u8]) -> Result<(usize, IpAddr, Port), Error> {
        // Allow receiving that arrived before the socket/connection was closed.
        if self.socket.state() != State::Connected && self.socket.state() != State::Closed {
            return Err(Error::new(Code::InvState));
        }

        self.socket
            .next_data(self.nm, data.len(), |buf, addr, port| {
                data[0..buf.len()].copy_from_slice(buf);
                (buf.len(), addr, port)
            })
    }

    pub fn send(&mut self, data: &[u8]) -> Result<(), Error> {
        self.send_to(data, IpAddr::unspecified(), 0)
    }

    pub fn send_to(&mut self, data: &[u8], addr: IpAddr, port: Port) -> Result<(), Error> {
        if self.socket.state() != State::Connected {
            return Err(Error::new(Code::InvState));
        }

        self.socket.send(self.nm, data, addr, port)
    }

    pub fn close(&mut self) -> Result<(), Error> {
        self.socket.close(self.nm)
    }

    pub fn abort(&mut self) -> Result<(), Error> {
        self.socket.abort(self.nm)
    }
}

impl Drop for TcpSocket<'_> {
    fn drop(&mut self) {
        // ignore errors
        self.abort().ok();
        self.nm.remove_socket(self.socket.sd());
    }
}
