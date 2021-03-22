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

use crate::errors::Error;
use crate::net::{
    socket::{DgramSocketArgs, Socket},
    Sd, SocketType,
};
use crate::rc::Rc;
use crate::session::NetworkManager;

/// Represents a raw internet protocol (IP) socket
pub struct RawSocket<'n> {
    socket: Rc<Socket>,
    nm: &'n NetworkManager,
}

impl<'n> RawSocket<'n> {
    pub fn new(args: DgramSocketArgs<'n>, protocol: Option<u8>) -> Result<Self, Error> {
        Ok(RawSocket {
            socket: args.nm.create(SocketType::Raw, protocol, &args.args)?,
            nm: args.nm,
        })
    }

    pub fn sd(&self) -> Sd {
        self.socket.sd()
    }
}

impl Drop for RawSocket<'_> {
    fn drop(&mut self) {
        self.nm.remove_socket(self.socket.sd());
    }
}
