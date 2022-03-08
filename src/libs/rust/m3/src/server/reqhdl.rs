/*
 * Copyright (C) 2020-2022 Nils Asmussen, Barkhausen Institut
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

use crate::cfg;
use crate::com::{GateIStream, RecvGate};
use crate::errors::Error;
use crate::math;
use crate::serialize::Unmarshallable;

/// The default maximum number of clients a service supports
pub const DEF_MAX_CLIENTS: usize = if cfg::MAX_ACTS < 32 {
    cfg::MAX_ACTS
}
else {
    32
};

/// The default message size used for the requests
pub const DEF_MSG_SIZE: usize = 64;

/// Handles requests from clients
pub struct RequestHandler {
    rgate: RecvGate,
}

impl RequestHandler {
    /// Creates a new request handler with default arguments
    pub fn default() -> Result<Self, Error> {
        Self::new_with(DEF_MAX_CLIENTS, DEF_MSG_SIZE)
    }

    /// Creates a new request handler for `max_clients` using a message size of `msg_size`.
    pub fn new_with(max_clients: usize, msg_size: usize) -> Result<Self, Error> {
        let mut rgate = RecvGate::new(
            math::next_log2(max_clients * msg_size),
            math::next_log2(msg_size),
        )?;
        rgate.activate()?;
        Ok(Self { rgate })
    }

    /// Returns the receive gate that is used to receive requests from clients
    pub fn recv_gate(&self) -> &RecvGate {
        &self.rgate
    }

    /// Fetches the next message from the receive gate and calls `func` in case there is a new
    /// message.
    ///
    /// The function `F` receives the opcode, which is expected to be the first value in the
    /// message, and the [`GateIStream`] for the message. The function `F` should return the result
    /// (success/failure) of the operation. In case of a failure, this function replies the error
    /// code. On success, it is expected that `func` sends the reply.
    pub fn handle<OP, F>(&self, mut func: F) -> Result<(), Error>
    where
        OP: Unmarshallable,
        F: FnMut(OP, &mut GateIStream<'_>) -> Result<(), Error>,
    {
        if let Some(msg) = self.rgate.fetch() {
            let mut is = GateIStream::new(msg, &self.rgate);
            if let Err(e) = is.pop::<OP>().and_then(|op| func(op, &mut is)) {
                // ignore errors here
                is.reply_error(e.code()).ok();
            }
        }
        Ok(())
    }
}
