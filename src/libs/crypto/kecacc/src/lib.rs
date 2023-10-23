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
// Note that this is a compile-time choice currently, the XKCP increases the
// binary size of HashMux by about 200 KiB.
// FIXME: Enable hardware accelerator on "hw" target
#[cfg(feature = "gem5")]
mod kecacc;

#[cfg(not(feature = "gem5"))]
#[path = "kecacc-xkcp.rs"]
mod kecacc;

pub use kecacc::{KecAcc, KecAccState};
