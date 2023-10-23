/*
 * Copyright (C) 2021, Stephan Gerhold <stephan.gerhold@mailbox.tu-dresden.de>
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

#![no_std]

// Choose between using hardware accelerator vs software fallback using XKCP.
// Currently this is a compile-time choice using Rust features.

#[cfg(not(feature = "backend-xkcp"))]
mod kecacc;

#[cfg(feature = "backend-xkcp")]
#[path = "kecacc-xkcp.rs"]
mod kecacc;

pub use kecacc::{KecAcc, KecAccState};
