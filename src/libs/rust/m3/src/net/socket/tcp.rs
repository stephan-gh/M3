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

pub struct StreamSocketArgs<'n> {
    nm: &'n NetworkManager,
    args: SocketArgs,
}

impl<'n> StreamSocketArgs<'n> {
    /// Creates a new `StreamSocketArgs` with default settings.
    pub fn new(nm: &'n NetworkManager) -> Self {
        Self {
            nm,
            args: SocketArgs::default(),
        }
    }

    /// Sets the size in bytes of the receive buffer
    pub fn recv_buffer(mut self, size: usize) -> Self {
        self.args.rbuf_size = size;
        self
    }

    /// Sets the size in bytes of the send buffer
    pub fn send_buffer(mut self, size: usize) -> Self {
        self.args.sbuf_size = size;
        self
    }
}

pub struct TcpSocket<'n> {
    socket: Rc<Socket>,
    nm: &'n NetworkManager,
}

impl<'n> TcpSocket<'n> {
    pub fn new(args: StreamSocketArgs<'n>) -> Result<Self, Error> {
        Ok(TcpSocket {
            socket: args.nm.create(SocketType::Stream, None, &args.args)?,
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

    pub fn listen(&mut self, port: Port) -> Result<(), Error> {
        if self.socket.state() != State::Closed {
            return Err(Error::new(Code::InvState));
        }

        let addr = self.nm.listen(self.socket.sd(), port)?;
        self.socket.set_local(addr, port, State::Listening);
        Ok(())
    }

    pub fn connect(&mut self, remote_addr: IpAddr, remote_port: Port) -> Result<(), Error> {
        self.socket.connect(self.nm, remote_addr, remote_port)
    }

    pub fn accept(&mut self) -> Result<(IpAddr, Port), Error> {
        self.socket.accept(self.nm)
    }

    pub fn has_data(&self) -> bool {
        self.socket.has_data()
    }

    pub fn recv(&mut self, data: &mut [u8]) -> Result<usize, Error> {
        match self.socket.state() {
            // receive is possible with an established connection or a connection that that has
            // already been closed by the remote side
            State::Connected | State::Closing => {
                self.socket
                    .next_data(self.nm, data.len(), |buf, _addr, _port| {
                        data[0..buf.len()].copy_from_slice(buf);
                        buf.len()
                    })
            },
            _ => Err(Error::new(Code::InvState)),
        }
    }

    pub fn send(&mut self, data: &[u8]) -> Result<(), Error> {
        match self.socket.state() {
            // like for receive: still allow sending if the remote side closed the connection
            State::Connected | State::Closing => {
                self.socket.send(self.nm, data, IpAddr::unspecified(), 0)
            },
            _ => Err(Error::new(Code::InvState)),
        }
    }

    pub fn close(&mut self) -> Result<(), Error> {
        self.socket.close(self.nm)
    }

    pub fn abort(&mut self) -> Result<(), Error> {
        self.socket.abort(self.nm, false)
    }
}

impl Drop for TcpSocket<'_> {
    fn drop(&mut self) {
        // ignore errors
        self.socket.abort(self.nm, true).ok();
        self.nm.remove_socket(self.socket.sd());
    }
}
