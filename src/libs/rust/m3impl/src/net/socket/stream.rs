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
use crate::net::{Endpoint, Port, Socket};

/// Trait for all stream sockets, like TCP.
pub trait StreamSocket: Socket {
    /// Puts this socket into listen mode on the given port.
    ///
    /// In listen mode, remote connections can be accepted. See [`accept`](StreamSocket::accept).
    /// Note that in contrast to conventional TCP/IP stacks, [`listen`](StreamSocket::listen) is a
    /// combination of the traditional `bind` and `listen`.
    ///
    /// Listing on this port requires that the used session has permission for this port. This is
    /// controlled with the "tcp=..." argument in the session argument of M³'s config files.
    ///
    /// Returns an error if the socket is not in state [`Closed`](crate::net::State::Closed).
    fn listen(&mut self, port: Port) -> Result<(), Error>;

    /// Accepts a remote connection on this socket
    ///
    /// The socket has to be put into listen mode first. Note that in contrast to conventional
    /// TCP/IP stacks, accept does not yield a new socket, but uses this socket for the accepted
    /// connection. Thus, to support multiple connections to the same port, put multiple sockets in
    /// listen mode on this port and call accept on each of them.
    fn accept(&mut self) -> Result<Endpoint, Error>;

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
