/*
 * Copyright (C) 2023 Nils Asmussen, Barkhausen Institut
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

//! Contains the opcode definitions for all protocols.

use num_enum::{IntoPrimitive, TryFromPrimitive};

use serde_repr::{Deserialize_repr, Serialize_repr};

#[derive(Copy, Clone, Debug, IntoPrimitive, TryFromPrimitive, Serialize_repr, Deserialize_repr)]
#[repr(usize)]
pub enum General {
    Connect = (1 << 31) + 0,
}

/// The operations for the file protocol.
#[derive(Copy, Clone, Debug, IntoPrimitive, TryFromPrimitive, Serialize_repr, Deserialize_repr)]
#[repr(usize)]
pub enum File {
    FStat,
    Seek,
    NextIn,
    NextOut,
    Commit,
    Truncate,
    Sync,
    CloneFile,
    GetPath,
    GetTMode,
    SetTMode,
    SetDest,
    EnableNotify,
    ReqNotify,
}

/// The operations for the file-system protocol.
#[derive(Copy, Clone, Debug, IntoPrimitive, TryFromPrimitive, Serialize_repr, Deserialize_repr)]
#[repr(usize)]
pub enum FileSystem {
    FStat        = File::FStat as usize,
    Seek         = File::Seek as usize,
    NextIn       = File::NextIn as usize,
    NextOut      = File::NextOut as usize,
    Commit       = File::Commit as usize,
    Truncate     = File::Truncate as usize,
    Sync         = File::Sync as usize,
    CloneFile    = File::CloneFile as usize,
    GetPath      = File::GetPath as usize,
    GetTMode     = File::GetTMode as usize,
    SetTMode     = File::SetTMode as usize,
    SetDest      = File::SetDest as usize,
    EnableNotify = File::EnableNotify as usize,
    ReqNotify    = File::ReqNotify as usize,
    Stat,
    Mkdir,
    Rmdir,
    Link,
    Unlink,
    Rename,
    Open,
    GetMem,
    DelEP,
    OpenPriv,
    ClosePriv,
    CloneMeta,
}

/// The operations for the pipe protocol.
#[derive(Copy, Clone, Debug, IntoPrimitive, TryFromPrimitive, Serialize_repr, Deserialize_repr)]
#[repr(usize)]
pub enum Pipe {
    FStat        = File::FStat as usize,
    Seek         = File::Seek as usize,
    NextIn       = File::NextIn as usize,
    NextOut      = File::NextOut as usize,
    Commit       = File::Commit as usize,
    CloneFile    = File::CloneFile as usize,
    SetDest      = File::SetDest as usize,
    EnableNotify = File::EnableNotify as usize,
    ReqNotify    = File::ReqNotify as usize,
    OpenPipe,
    OpenChan,
    SetMem,
}

/// The operations for the network protocol.
#[derive(Copy, Clone, Debug, IntoPrimitive, TryFromPrimitive, Serialize_repr, Deserialize_repr)]
#[repr(usize)]
pub enum Net {
    Bind,
    Listen,
    Connect,
    Abort,
    Create,
    GetIP,
    GetNameSrv,
}

/// The operations for the resmng protocol.
#[derive(Copy, Clone, Debug, IntoPrimitive, TryFromPrimitive, Serialize_repr, Deserialize_repr)]
#[repr(usize)]
pub enum ResMng {
    RegServ,
    UnregServ,
    OpenSess,
    CloseSess,
    AddChild,
    RemChild,
    AllocMem,
    FreeMem,
    AllocTile,
    FreeTile,
    UseRGate,
    UseSGate,
    UseSem,
    UseMod,
    GetSerial,
    GetInfo,
}

/// The operations for the pager protocol.
#[derive(Copy, Clone, Debug, IntoPrimitive, TryFromPrimitive, Serialize_repr, Deserialize_repr)]
#[repr(usize)]
pub enum Pager {
    /// A page fault
    Pagefault,
    /// Initializes the pager session
    Init,
    /// Adds a child activity to the pager session
    AddChild,
    /// Clone the address space of a child activity (see `ADD_CHILD`) from the parent
    Clone,
    /// Add a new mapping with anonymous memory
    MapAnon,
    /// Add a new data space mapping (e.g., a file)
    MapDS,
    /// Add a new mapping for a given memory capability
    MapMem,
    /// Remove an existing mapping
    Unmap,
}

/// The operations for the disk protocol.
#[derive(Copy, Clone, Debug, IntoPrimitive, TryFromPrimitive, Serialize_repr, Deserialize_repr)]
#[repr(usize)]
pub enum Disk {
    Read,
    Write,
    AddMem,
}

/// The operations for the hash protocol.
#[derive(Copy, Clone, Debug, IntoPrimitive, TryFromPrimitive, Serialize_repr, Deserialize_repr)]
#[repr(usize)]
pub enum Hash {
    Reset,
    Input,
    Output,
    GetMem,
}
