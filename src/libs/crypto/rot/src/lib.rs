/*
 * Copyright (C) 2023-2024, Stephan Gerhold <stephan@gerhold.net>
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

pub use {ed25519_dalek as ed25519, serde_json as json};

pub use bin::*;
pub use cdi::*;
pub use cfg::*;
pub use ctx::*;
pub use hex::*;
pub use hw::*;
pub use secret::*;
pub use {cshake, kecacc};

pub type Magic = u64;

pub const fn encode_magic(id: &[u8; 7], version: u8) -> Magic {
    u64::from_be_bytes([id[0], id[1], id[2], id[3], id[4], id[5], id[6], version])
}

#[cfg(target_arch = "riscv64")]
mod asm;

mod bin;
mod cdi;
pub mod cert;
mod cfg;
mod ctx;
mod hex;
mod hw;
mod secret;
