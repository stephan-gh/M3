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

use core::fmt;

use crate::errors::Error;
use crate::io;
use crate::net::{
    socket::{DgramSocketArgs, Socket},
    Endpoint, SocketType,
};
use crate::rc::Rc;
use crate::session::{HashInput, HashOutput, NetworkManager};
use crate::vfs::{self, Fd, File};

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

impl File for RawSocket<'_> {
    fn fd(&self) -> Fd {
        self.socket.sd()
    }

    fn set_fd(&mut self, _fd: Fd) {
        // not used
    }

    fn file_type(&self) -> u8 {
        // not supported
        b'\0'
    }

    fn is_blocking(&self) -> bool {
        self.socket.blocking()
    }

    /// Sets whether the socket is using blocking mode.
    ///
    /// In blocking mode, all functions ([`send`](RawSocket::send), [`recv`](RawSocket::recv), ...)
    /// do not return until the operation is complete. In non-blocking mode, all functions return in
    /// case they would need to block, that is, wait until an event is received or further data can
    /// be sent.
    fn set_blocking(&mut self, blocking: bool) -> Result<(), Error> {
        self.socket.set_blocking(blocking);
        Ok(())
    }
}

impl io::Read for RawSocket<'_> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        self.recv(buf)
    }
}

impl io::Write for RawSocket<'_> {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Error> {
        self.send(buf).map(|_| buf.len())
    }
}

impl vfs::Seek for RawSocket<'_> {
}

impl vfs::Map for RawSocket<'_> {
}

impl HashInput for RawSocket<'_> {
}

impl HashOutput for RawSocket<'_> {
}

impl fmt::Debug for RawSocket<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RawSocket")
    }
}

impl Drop for RawSocket<'_> {
    fn drop(&mut self) {
        self.socket.tear_down();
        self.nm.remove_socket(self.socket.sd());
    }
}
