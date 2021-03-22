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
    socket::{Socket, SocketArgs, State},
    IpAddr, Port, Sd, SocketType,
};
use crate::rc::Rc;
use crate::session::NetworkManager;

pub struct DgramSocketArgs<'n> {
    pub(crate) nm: &'n NetworkManager,
    pub(crate) args: SocketArgs,
}

impl<'n> DgramSocketArgs<'n> {
    /// Creates a new `DgramSocketArgs` with default settings.
    pub fn new(nm: &'n NetworkManager) -> Self {
        Self {
            nm,
            args: SocketArgs::default(),
        }
    }

    /// Sets the number of slots and the size in bytes of the receive buffer
    pub fn recv_buffer(mut self, slots: usize, size: usize) -> Self {
        self.args.rbuf_slots = slots;
        self.args.rbuf_size = size;
        self
    }

    /// Sets the number of slots and the size in bytes of the send buffer
    pub fn send_buffer(mut self, slots: usize, size: usize) -> Self {
        self.args.sbuf_slots = slots;
        self.args.sbuf_size = size;
        self
    }
}

pub struct UdpSocket<'n> {
    socket: Rc<Socket>,
    nm: &'n NetworkManager,
}

impl<'n> UdpSocket<'n> {
    pub fn new(args: DgramSocketArgs<'n>) -> Result<Self, Error> {
        Ok(UdpSocket {
            socket: args.nm.create(SocketType::Dgram, None, &args.args)?,
            nm: args.nm,
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

    pub fn bind(&mut self, port: Port) -> Result<(), Error> {
        if self.socket.state() != State::Closed {
            return Err(Error::new(Code::InvState));
        }

        let addr = self.nm.bind(self.socket.sd(), port)?;
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
        self.socket.abort(self.nm, false)
    }
}

impl Drop for UdpSocket<'_> {
    fn drop(&mut self) {
        // ignore errors
        self.socket.abort(self.nm, true).ok();
        self.nm.remove_socket(self.socket.sd());
    }
}
