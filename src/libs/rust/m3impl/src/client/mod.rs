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

//! Contains client-side abstractions
//!
//! This module contains abstraction to interact with the services that are provided by M³. These
//! are based on a [`ClientSession`] that refers to a corresponding session at the server side,
//! potentially holding client-specific state. Based on the [`ClientSession`], capabilities can be
//! exchanged to establish further communication channels.
//!
//! The [`ClientSession`] is therefore used to provide service-specific APIs on top. For example,
//! [`Pipes`] builds upon a [`ClientSession`] and uses it to perform capability exchanges in order
//! to create pipes and channels to such pipes.

mod disk;
mod hash;
mod m3fs;
mod network;
mod pager;
mod pipe;
pub mod resmng;
mod rot;
mod session;
mod vterm;

pub use self::disk::{Disk, DiskBlockNo, DiskBlockRange};
pub use self::hash::{HashInput, HashOutput, HashSession};
pub use self::m3fs::M3FS;
pub use self::network::Network;
pub use self::pager::{MapFlags, Pager};
pub use self::pipe::{Pipe, Pipes};
pub use self::resmng::{ResMng, ResMngChild};
pub use self::rot::RoTSession;
pub use self::session::ClientSession;
pub use self::vterm::VTerm;
