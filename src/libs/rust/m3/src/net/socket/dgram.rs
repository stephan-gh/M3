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

/// Trait for all data-gram sockets, like UDP.
pub trait DGramSocket {
    /// Returns the current state of the socket
    fn state(&self) -> State;

    /// Returns the local endpoint
    ///
    /// The local endpoint is only `Some` if the socket has been bound via
    /// [`bind`](DGramSocket::bind).
    fn local_endpoint(&self) -> Option<Endpoint>;

    /// Binds this socket to the given local port.
    ///
    /// Note that specifying 0 for `port` will allocate an ephemeral port for this socket.
    ///
    /// Receiving packets from remote endpoints requires a call to bind before. For sending packets,
    /// bind(0) is called implicitly to bind the socket to a local ephemeral port.
    ///
    /// Binding to a specific (non-zero) port requires that the used session has permission for this
    /// port. This is controlled with the "udp=..." argument in the session argument of M³'s config
    /// files.
    ///
    /// Returns an error if the socket is not in state [`Closed`](State::Closed).
    fn bind(&mut self, port: Port) -> Result<(), Error>;

    /// Connects this socket to the given remote endpoint.
    ///
    /// Note that this merely sets the endpoint to use for subsequent send calls and therefore does
    /// not involve the remote side in any way.
    ///
    /// If the socket has not been bound so far, bind(0) will be called to bind it to an unused
    /// ephemeral port.
    fn connect(&mut self, ep: Endpoint) -> Result<(), Error>;

    /// Returns whether data can currently be received from the socket
    ///
    /// Note that this function does not process events. To receive data, any receive function on
    /// this socket or [`FileWaiter::wait`](crate::vfs::FileWaiter::wait) has to be called.
    fn has_data(&self) -> bool;

    /// Receives data from the socket into the given buffer.
    ///
    /// Returns the number of received bytes.
    fn recv(&mut self, data: &mut [u8]) -> Result<usize, Error>;

    /// Receives data from the socket into the given buffer.
    ///
    /// Returns the number of received bytes and the remote endpoint it was received from.
    fn recv_from(&mut self, data: &mut [u8]) -> Result<(usize, Endpoint), Error>;

    /// Sends the given data to the remote endpoint set at connect.
    ///
    /// This function fails with `Code::InvState` if connect has not been called before.
    ///
    /// If the socket has not been bound so far, bind(0) will be called to bind it to an unused
    /// ephemeral port.
    fn send(&mut self, data: &[u8]) -> Result<(), Error>;

    /// Sends the given data to the given remote endpoint
    ///
    /// If the socket has not been bound so far, bind(0) will be called to bind it to an unused
    /// ephemeral port.
    fn send_to(&mut self, data: &[u8], endpoint: Endpoint) -> Result<(), Error>;
}
