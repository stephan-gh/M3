/*
 * Copyright (C) 2022 Nils Asmussen, Barkhausen Institut
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
use crate::net::{socket::State, Endpoint, Port};

/// Trait for all stream sockets, like TCP.
pub trait StreamSocket {
    /// Returns the current state of the socket
    fn state(&self) -> State;

    /// Returns the local endpoint
    ///
    /// The local endpoint is only `Some` if the socket has been put into listen mode via
    /// [`listen`](StreamSocket::listen) or was connected to a remote endpoint via
    /// [`connect`](StreamSocket::connect).
    fn local_endpoint(&self) -> Option<Endpoint>;

    /// Returns the remote endpoint
    ///
    /// The remote endpoint is only `Some`, if the socket is currently connected (achieved either
    /// via [`connect`](StreamSocket::connect) or [`accept`](StreamSocket::accept)). Otherwise, the
    /// remote endpoint is `None`.
    fn remote_endpoint(&self) -> Option<Endpoint>;

    /// Puts this socket into listen mode on the given port.
    ///
    /// In listen mode, remote connections can be accepted. See [`accept`](StreamSocket::accept).
    /// Note that in contrast to conventional TCP/IP stacks, [`listen`](StreamSocket::listen) is a
    /// combination of the traditional `bind` and `listen`.
    ///
    /// Listing on this port requires that the used session has permission for this port. This is
    /// controlled with the "tcp=..." argument in the session argument of M³'s config files.
    ///
    /// Returns an error if the socket is not in state [`Closed`](State::Closed).
    fn listen(&mut self, port: Port) -> Result<(), Error>;

    /// Connects this socket to the given remote endpoint.
    fn connect(&mut self, endpoint: Endpoint) -> Result<(), Error>;

    /// Accepts a remote connection on this socket
    ///
    /// The socket has to be put into listen mode first. Note that in contrast to conventional
    /// TCP/IP stacks, accept does not yield a new socket, but uses this socket for the accepted
    /// connection. Thus, to support multiple connections to the same port, put multiple sockets in
    /// listen mode on this port and call accept on each of them.
    fn accept(&mut self) -> Result<Endpoint, Error>;

    /// Returns whether data can currently be received from the socket
    ///
    /// Note that this function does not process events. To receive data, any receive function on
    /// this socket or [`FileWaiter::wait`](crate::vfs::FileWaiter::wait) has to be called.
    fn has_data(&self) -> bool;

    /// Receives data from the socket into the given buffer.
    ///
    /// The socket has to be connected first (either via [`connect`](StreamSocket::connect) or
    /// [`accept`](StreamSocket::accept)). Note that data can be received after the remote side has
    /// closed the socket (state [`RemoteClosed`](State::RemoteClosed)), but not if this side has
    /// been closed.
    ///
    /// Returns the number of received bytes.
    fn recv(&mut self, data: &mut [u8]) -> Result<usize, Error>;

    /// Sends the given data to this socket
    ///
    /// The socket has to be connected first (either via [`connect`](StreamSocket::connect) or
    /// [`accept`](StreamSocket::accept)). Note that data can be received after the remote side has
    /// closed the socket (state [`RemoteClosed`](State::RemoteClosed)), but not if this side has
    /// been closed.
    ///
    /// Returns the number of sent bytes or an error. If an error occurs (e.g., remote side closed
    /// the socket) and some of the data has already been sent, the number of sent bytes is
    /// returned. Otherwise, the error is returned.
    fn send(&mut self, data: &[u8]) -> Result<usize, Error>;

    /// Closes the connection
    ///
    /// In contrast to [`abort`](StreamSocket::abort), close properly closes the connection to the
    /// remote endpoint by going through the TCP protocol.
    ///
    /// Note that [`close`](StreamSocket::close) is also called on drop.
    fn close(&mut self) -> Result<(), Error>;

    /// Aborts the connection
    ///
    /// In contrast to [`close`](StreamSocket::close), this is a hard abort, which does not go
    /// through the TCP protocol, but simply "forgets" this socket. Furthermore, it is *not*
    /// guaranteed that all data has already been transmitted. Use [`close`](StreamSocket::close) if
    /// that is important.
    fn abort(&mut self) -> Result<(), Error>;
}
