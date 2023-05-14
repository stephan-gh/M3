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

use crate::client::Network;
use crate::errors::Error;
use crate::net::{Endpoint, Port, Socket, SocketArgs};
use crate::rc::Rc;

/// Configures the buffer sizes for [`DGramSocket`]s
pub struct DgramSocketArgs {
    pub(crate) net: Rc<Network>,
    pub(crate) args: SocketArgs,
}

impl DgramSocketArgs {
    /// Creates a new [`DgramSocketArgs`] with default settings
    pub fn new(net: Rc<Network>) -> Self {
        Self {
            net,
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

/// Trait for all data-gram sockets, like UDP
pub trait DGramSocket: Socket {
    /// Binds this socket to the given local port
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
    /// Returns an error if the socket is not in state [`Closed`](crate::net::State::Closed).
    fn bind(&mut self, port: Port) -> Result<(), Error>;

    /// Receives data from the socket into the given buffer
    ///
    /// Returns the number of received bytes and the remote endpoint it was received from.
    fn recv_from(&mut self, data: &mut [u8]) -> Result<(usize, Endpoint), Error>;

    /// Sends the given data to the given remote endpoint
    ///
    /// If the socket has not been bound so far, bind(0) will be called to bind it to an unused
    /// ephemeral port.
    fn send_to(&mut self, data: &[u8], endpoint: Endpoint) -> Result<(), Error>;
}
