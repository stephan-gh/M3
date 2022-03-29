/*
 * Copyright (C) 2021-2022 Nils Asmussen, Barkhausen Institut
 * Copyright (C) 2021, Tendsin Mende <tendsin.mende@mailbox.tu-dresden.de>
 * Copyright (C) 2017, Georg Kotheimer <georg.kotheimer@mailbox.tu-dresden.de>
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
use crate::net::Endpoint;
use crate::net::{
    socket::{DgramSocketArgs, Socket},
    Sd, SocketType,
};
use crate::rc::Rc;
use crate::session::NetworkManager;

pub type RawSocketArgs<'a> = DgramSocketArgs<'a>;

/// Represents a raw internet protocol (IP) socket
pub struct RawSocket<'n> {
    socket: Rc<Socket>,
    nm: &'n NetworkManager,
}

impl<'n> RawSocket<'n> {
    /// Creates a new raw IP socket with given arguments.
    ///
    /// By default, the socket is in blocking mode, that is, all functions
    /// ([`send`](RawSocket::send), [`recv`](RawSocket::recv), ...) do not return until the
    /// operation is complete. This can be changed via [`set_blocking`](RawSocket::set_blocking).
    ///
    /// Creation of a raw socket requires that the used session has permission to do so. This is
    /// controlled with the "raw=yes" argument in the session argument of M³'s config files.
    pub fn new(args: RawSocketArgs<'n>, protocol: Option<u8>) -> Result<Self, Error> {
        Ok(RawSocket {
            socket: args.nm.create(SocketType::Raw, protocol, &args.args)?,
            nm: args.nm,
        })
    }

    /// Returns the socket descriptor used to identify this socket within the session on the server
    pub fn sd(&self) -> Sd {
        self.socket.sd()
    }

    /// Returns whether the socket is currently in blocking mode
    pub fn blocking(&self) -> bool {
        self.socket.blocking()
    }

    /// Sets whether the socket is using blocking mode.
    ///
    /// In blocking mode, all functions ([`send`](RawSocket::send), [`recv`](RawSocket::recv), ...)
    /// do not return until the operation is complete. In non-blocking mode, all functions return in
    /// case they would need to block, that is, wait until an event is received or further data can
    /// be sent.
    pub fn set_blocking(&mut self, blocking: bool) {
        self.socket.set_blocking(blocking);
    }

    /// Returns whether data can currently be received from the socket
    ///
    /// Note that this function does not process events. To receive data, any receive function on
    /// this socket or [`NetworkManager::wait`] has to be called.
    pub fn has_data(&self) -> bool {
        self.socket.has_data()
    }

    /// Receives data from the socket into the given buffer.
    ///
    /// Returns the number of received bytes.
    pub fn recv(&self, data: &mut [u8]) -> Result<usize, Error> {
        self.socket.next_data(data.len(), |buf, _ep| {
            data[0..buf.len()].copy_from_slice(buf);
            (buf.len(), buf.len())
        })
    }

    /// Sends the given data to the given remote endpoint
    pub fn send(&self, data: &[u8]) -> Result<(), Error> {
        self.socket.send(data, Endpoint::unspecified())
    }
}

impl Drop for RawSocket<'_> {
    fn drop(&mut self) {
        self.socket.tear_down();
        self.nm.remove_socket(self.socket.sd());
    }
}
