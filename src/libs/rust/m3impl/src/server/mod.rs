/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2022 Nils Asmussen, Barkhausen Institut
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

//! Contains server-related abstractions.
//!
//! The foundation of the server API is provided by [`Server`], which is responsible for handling
//! the interaction with the kernel (service registration, session creation, capability exchanges,
//! etc.). [`Server`] is customized by a [`Handler`] trait, which defines how sessions are opened
//! and closed and is responsible for capability exchanges over these sessions. The typically used
//! implementation of [`Handler`] is [`RequestHandler`], which implements the way we handle client
//! requests in pretty much all servers and supports the registration of capability handlers and
//! message handlers.

mod reqhdl;
#[allow(clippy::module_inception)]
mod server;
mod sesscon;
mod session;

pub use self::reqhdl::{
    ClientManager, RequestHandler, RequestSession, DEF_MAX_CLIENTS, DEF_MSG_SIZE,
};
pub use self::server::{CapExchange, ExcType, Handler, Server};
pub use self::sesscon::{SessId, SessionContainer};
pub use self::session::ServerSession;

use crate::errors::Error;
use crate::tiles::OwnActivity;

/// Executes the server loop, calling `func` in every iteration.
pub fn server_loop<F: FnMut() -> Result<(), Error>>(mut func: F) -> Result<(), Error> {
    loop {
        OwnActivity::sleep().ok();

        func()?;
    }
}
