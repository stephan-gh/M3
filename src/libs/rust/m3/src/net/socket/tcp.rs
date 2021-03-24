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

/// Configures the buffer sizes for stream sockets
pub struct StreamSocketArgs<'n> {
    nm: &'n NetworkManager,
    args: SocketArgs,
}

impl<'n> StreamSocketArgs<'n> {
    /// Creates a new [`StreamSocketArgs`] with default settings
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

/// Represents a stream socket using the transmission control protocol (TCP)
pub struct TcpSocket<'n> {
    socket: Rc<Socket>,
    nm: &'n NetworkManager,
}

impl<'n> TcpSocket<'n> {
    /// Creates a new TCP sockets with given arguments.
    ///
    /// By default, the socket is in blocking mode, that is, all functions
    /// ([`connect`](TcpSocket::connect), [`send`](TcpSocket::send), [`recv`](TcpSocket::recv),
    /// ...) do not return until the operation is complete. This can be changed via
    /// [`set_blocking`](TcpSocket::set_blocking).
    pub fn new(args: StreamSocketArgs<'n>) -> Result<Self, Error> {
        Ok(TcpSocket {
            socket: args.nm.create(SocketType::Stream, None, &args.args)?,
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
    /// In blocking mode, all functions ([`connect`](TcpSocket::connect), [`send`](TcpSocket::send),
    /// [`recv`](TcpSocket::recv), ...) do not return until the operation is complete. In
    /// non-blocking mode, all functions return in case they would need to block, that is, wait
    /// until an event is received or further data can be sent.
    pub fn set_blocking(&mut self, blocking: bool) {
        self.socket.set_blocking(blocking);
    }

    /// Puts this socket into listen mode on the given port.
    ///
    /// In listen mode, remote connections can be accepted. See [`accept`](TcpSocket::accept). Note
    /// that in contrast to conventional TCP/IP stacks, [`listen`](TcpSocket::listen) is a
    /// combination of the traditional `bind` and `listen`.
    ///
    /// Listing on this port requires that the used session has permission for this port. This is
    /// controlled with the "ports=..." argument in the session argument of M³'s config files.
    ///
    /// Returns an error if the socket is not in state [`Closed`](State::Closed).
    pub fn listen(&mut self, port: Port) -> Result<(), Error> {
        if self.socket.state() != State::Closed {
            return Err(Error::new(Code::InvState));
        }

        let addr = self.nm.listen(self.socket.sd(), port)?;
        self.socket.set_local(addr, port, State::Listening);
        Ok(())
    }

    /// Connects this socket to the given remote endpoint.
    pub fn connect(&mut self, remote_addr: IpAddr, remote_port: Port) -> Result<(), Error> {
        self.socket.connect(self.nm, remote_addr, remote_port)
    }

    /// Accepts a remote connection on this socket
    ///
    /// The socket has to be put into listen mode first. Note that in contrast to conventional
    /// TCP/IP stacks, accept does not yield a new socket, but uses this socket for the accepted
    /// connection. Thus, to support multiple connections to the same port, put multiple sockets in
    /// listen mode on this port and call accept on each of them.
    pub fn accept(&mut self) -> Result<(IpAddr, Port), Error> {
        self.socket.accept(self.nm)
    }

    /// Returns whether data can currently be received from the socket
    pub fn has_data(&self) -> bool {
        self.socket.has_data()
    }

    /// Receives data from the socket into the given buffer.
    ///
    /// The socket has to be connected first (either via [`connect`](TcpSocket::connect) or
    /// [`accept`](TcpSocket::accept)). Note that data can be received after the remote side has
    /// closed the socket (state [`Closing`](State::Closing)), but not if this side has been closed.
    ///
    /// Returns the number of received bytes.
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
            _ => Err(Error::new(Code::NotConnected)),
        }
    }

    /// Sends the given data to this socket
    ///
    /// The socket has to be connected first (either via [`connect`](TcpSocket::connect) or
    /// [`accept`](TcpSocket::accept)). Note that data can be received after the remote side has
    /// closed the socket (state [`Closing`](State::Closing)), but not if this side has been closed.
    pub fn send(&mut self, data: &[u8]) -> Result<(), Error> {
        match self.socket.state() {
            // like for receive: still allow sending if the remote side closed the connection
            State::Connected | State::Closing => self.socket.send(
                self.nm,
                data,
                self.socket.remote_addr.get(),
                self.socket.remote_port.get(),
            ),
            _ => Err(Error::new(Code::NotConnected)),
        }
    }

    /// Closes the connection
    ///
    /// In contrast to [`abort`](TcpSocket::abort), close properly closes the connection to the
    /// remote endpoint by going through the TCP protocol.
    ///
    /// Note that [`close`](TcpSocket::close) is *not* called on drop, but has to be called
    /// explicitly to ensure that all data is transmitted to the remote end and the connection is
    /// properly closed.
    pub fn close(&mut self) -> Result<(), Error> {
        self.socket.close(self.nm)
    }

    /// Aborts the connection
    ///
    /// In contrast to [`close`](TcpSocket::close), this is a hard abort, which does not go through
    /// the TCP protocol, but simply "forgets" this socket. Furthermore, it is *not* guaranteed that
    /// all data has already been transmitted. Use [`close`](TcpSocket::close) if that is important.
    ///
    /// Note also that [`abort`](TcpSocket::abort) is called automatically on drop.
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
