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

/// Configures the buffer sizes for datagram sockets
pub struct DgramSocketArgs<'n> {
    pub(crate) nm: &'n NetworkManager,
    pub(crate) args: SocketArgs,
}

impl<'n> DgramSocketArgs<'n> {
    /// Creates a new [`DgramSocketArgs`] with default settings.
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

/// Represents a datagram socket using the user datagram protocol (UDP)
pub struct UdpSocket<'n> {
    socket: Rc<Socket>,
    nm: &'n NetworkManager,
}

impl<'n> UdpSocket<'n> {
    /// Creates a new UDP sockets with given arguments.
    ///
    /// By default, the socket is in blocking mode, that is, all functions
    /// ([`send_to`](UdpSocket::send_to), [`recv_from`](UdpSocket::recv_from), ...) do not return
    /// until the operation is complete. This can be changed via
    /// [`set_blocking`](UdpSocket::set_blocking).
    pub fn new(args: DgramSocketArgs<'n>) -> Result<Self, Error> {
        Ok(UdpSocket {
            socket: args.nm.create(SocketType::Dgram, None, &args.args)?,
            nm: args.nm,
        })
    }

    /// Returns the socket descriptor used to identify this socket within the session on the server
    pub fn sd(&self) -> Sd {
        self.socket.sd()
    }

    /// Returns the current state of the socket
    pub fn state(&self) -> State {
        self.socket.state()
    }

    /// Returns whether the socket is currently in blocking mode
    pub fn blocking(&self) -> bool {
        self.socket.blocking()
    }

    /// Sets whether the socket is using blocking mode.
    ///
    /// In blocking mode, all functions ([`send_to`](UdpSocket::send_to),
    /// [`recv_from`](UdpSocket::recv_from), ...) do not return until the operation is complete. In
    /// non-blocking mode, all functions return in case they would need to block, that is, wait
    /// until an event is received or further data can be sent.
    pub fn set_blocking(&mut self, blocking: bool) {
        self.socket.set_blocking(blocking);
    }

    /// Binds this socket to the given local port.
    ///
    /// When bound, packets can be received from remote endpoints.
    ///
    /// Binding requires that the used session has permission for this port. This is controlled with
    /// the "ports=..." argument in the session argument of M³'s config files.
    ///
    /// Returns an error if the socket is not in state [`Closed`](State::Closed).
    pub fn bind(&mut self, port: Port) -> Result<(), Error> {
        if self.socket.state() != State::Closed {
            return Err(Error::new(Code::InvState));
        }

        let addr = self.nm.bind(self.socket.sd(), port)?;
        self.socket.set_local(addr, port, State::Bound);
        Ok(())
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
        self.recv_from(data).map(|(size, _, _)| size)
    }

    /// Receives data from the socket into the given buffer.
    ///
    /// Returns the number of received bytes and the remote endpoint it was received from.
    pub fn recv_from(&self, data: &mut [u8]) -> Result<(usize, IpAddr, Port), Error> {
        self.socket.next_data(data.len(), |buf, addr, port| {
            data[0..buf.len()].copy_from_slice(buf);
            (buf.len(), addr, port)
        })
    }

    /// Sends the given data to the given remote endpoint
    pub fn send_to(&self, data: &[u8], addr: IpAddr, port: Port) -> Result<(), Error> {
        self.socket.send(data, addr, port)
    }
}

impl Drop for UdpSocket<'_> {
    fn drop(&mut self) {
        // we have no connection to tear down here, but only want to make sure that all packets we
        // sent are seen and handled by the server. thus, wait until we have got all replies to our
        // potentially in-flight packets, in which case we also have received our credits back.
        while !self.socket.has_all_credits() {
            self.socket.wait_for_credits();
        }

        self.nm.remove_socket(self.socket.sd());
    }
}
