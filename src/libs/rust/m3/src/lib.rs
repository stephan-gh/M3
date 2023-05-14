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

//! The `m3` library is the standard library for M³ applications and services.
//!
//! In contrast to the [`base`](../base/index.html) library, the `m3` library depends on the
//! presence of the M³ kernel and TileMux. It is therefore used by applications and services, but
//! not by the M³ kernel, for example. Additionally, the `m3` library builds upon the `base` library
//! and re-exports its types.
//!
//! The following picture provides an overview of the components that the `m3` library interacts
//! with:
//!
//! ```text
//! +-----------+     +----------------+     +---------+
//! |           |     | M³ app/service |     |         |
//! | M³ kernel |     +----------------+     | TileMux |
//! |           | <-> |     libm3      | <-> |         |
//! +-----------+     +----------------+     +---------+
//! |  libbase  |     |    libbase     |     | libbase |
//! +-----------+     +----------------+     +---------+
//! ```
//!
//! As illustrated above, applications and services (*activities*) interact with the M³ kernel,
//! TileMux, and potentially other applications and services using the `m3` library. The M³ kernel
//! manages the capabilities of all activities and offers *system calls* to create, exchange, and
//! revoke capabilities. The `m3` library therefore sends system calls to the M³ kernel to work with
//! capabilities.
//!
//! Additionally, the `m3` library interacts with TileMux, the tile-local multiplexer. TileMux is
//! responsible for the architecture-specific runtime support (startup, exceptions handling, exit,
//! etc.) and switches between multiple activities on the same tile. As such, the `m3` library uses
//! TileMux to voluntarily block until the next message reception and to handle TCU-TLB misses.
//!
//! On top of these rather low-level interactions with other components, the `m3` library provides
//! several abstractions:
//! - [`communication`](`crate::com`): endpoints, gates, channels, semaphores, and IPC streams
//! - [`input/output`](`crate::io`): stdin, stdout, stderr, println, etc.
//! - [`networking`](`crate::net`): sockets (TCP/UDP) an DNS resolver
//! - [`client`](`crate::client`): client-side APIs for the available M³ services
//! - [`server`](`crate::server`): request handling, session management, etc.
//! - [`tiles`](`crate::tiles`): tiles and activities on tiles
//! - [`vfs`](`crate::vfs`): virtual file system

#![no_std]

#[allow(unused_extern_crates)]
extern crate heap;

#[allow(unused_extern_crates)]
extern crate lang;

pub use m3impl::*;
