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

use crate::int_enum;

int_enum! {
    /// The operations for the file protocol.
    pub struct File : u64 {
        const STAT          = 0;
        const SEEK          = 1;
        const NEXT_IN       = 2;
        const NEXT_OUT      = 3;
        const COMMIT        = 4;
        const TRUNCATE      = 5;
        const SYNC          = 6;
        const CLOSE         = 7;
        const CLONE         = 8;
        const GET_PATH      = 9;
        const GET_TMODE     = 10;
        const SET_TMODE     = 11;
        const SET_DEST      = 12;
        const ENABLE_NOTIFY = 13;
        const REQ_NOTIFY    = 14;
    }
}

int_enum! {
    /// The operations for the file-system protocol.
    pub struct FileSystem : u64 {
        const STAT          = File::REQ_NOTIFY.val + 1;
        const MKDIR         = File::REQ_NOTIFY.val + 2;
        const RMDIR         = File::REQ_NOTIFY.val + 3;
        const LINK          = File::REQ_NOTIFY.val + 4;
        const UNLINK        = File::REQ_NOTIFY.val + 5;
        const RENAME        = File::REQ_NOTIFY.val + 6;
        const OPEN          = File::REQ_NOTIFY.val + 7;
        const GET_SGATE     = File::REQ_NOTIFY.val + 8;
        const GET_MEM       = File::REQ_NOTIFY.val + 9;
        const DEL_EP        = File::REQ_NOTIFY.val + 10;
        const OPEN_PRIV     = File::REQ_NOTIFY.val + 11;
    }
}

int_enum! {
    /// The operations for the pipe protocol.
    pub struct Pipe : u64 {
        const OPEN_PIPE     = File::REQ_NOTIFY.val + 1;
        const OPEN_CHAN     = File::REQ_NOTIFY.val + 2;
        const SET_MEM       = File::REQ_NOTIFY.val + 3;
        const CLOSE_PIPE    = File::REQ_NOTIFY.val + 4;
    }
}

int_enum! {
    /// The operations for the network protocol.
    pub struct Net : u64 {
        const STAT          = File::STAT.val;
        const SEEK          = File::SEEK.val;
        const NEXT_IN       = File::NEXT_IN.val;
        const NEXT_OUT      = File::NEXT_OUT.val;
        const COMMIT        = File::COMMIT.val;
        const TRUNCATE      = File::TRUNCATE.val;
        // TODO what about GenericFile::CLOSE?
        const BIND          = File::REQ_NOTIFY.val + 1;
        const LISTEN        = File::REQ_NOTIFY.val + 2;
        const CONNECT       = File::REQ_NOTIFY.val + 3;
        const ABORT         = File::REQ_NOTIFY.val + 4;
        const CREATE        = File::REQ_NOTIFY.val + 5;
        const GET_IP        = File::REQ_NOTIFY.val + 6;
        const GET_NAMESRV   = File::REQ_NOTIFY.val + 7;
        const GET_SGATE     = File::REQ_NOTIFY.val + 8;
        const OPEN_FILE     = File::REQ_NOTIFY.val + 9;
    }
}

int_enum! {
    /// The operations for the resmng protocol.
    pub struct ResMng : u64 {
        const REG_SERV      = 0;
        const UNREG_SERV    = 1;

        const OPEN_SESS     = 2;
        const CLOSE_SESS    = 3;

        const ADD_CHILD     = 4;
        const REM_CHILD     = 5;

        const ALLOC_MEM     = 6;
        const FREE_MEM      = 7;

        const ALLOC_TILE    = 8;
        const FREE_TILE     = 9;

        const USE_RGATE     = 10;
        const USE_SGATE     = 11;

        const USE_SEM       = 12;
        const USE_MOD       = 13;

        const GET_SERIAL    = 14;

        const GET_INFO      = 15;
    }
}

int_enum! {
    /// The operations for the pager protocol.
    pub struct Pager : u64 {
        /// A page fault
        const PAGEFAULT     = 0;
        /// Initializes the pager session
        const INIT          = 1;
        /// Adds a child activity to the pager session
        const ADD_CHILD     = 2;
        /// Adds a new send gate to the pager session
        const ADD_SGATE     = 3;
        /// Clone the address space of a child activity (see `ADD_CHILD`) from the parent
        const CLONE         = 4;
        /// Add a new mapping with anonymous memory
        const MAP_ANON      = 5;
        /// Add a new data space mapping (e.g., a file)
        const MAP_DS        = 6;
        /// Add a new mapping for a given memory capability
        const MAP_MEM       = 7;
        /// Remove an existing mapping
        const UNMAP         = 8;
        /// Close the pager session
        const CLOSE         = 9;
    }
}

int_enum! {
    /// The operations for the disk protocol.
    pub struct Disk : u64 {
        const READ          = 0;
        const WRITE         = 1;
    }
}

int_enum! {
    /// The operations for the hash protocol.
    pub struct Hash : u64 {
        const RESET         = 0;
        const INPUT         = 1;
        const OUTPUT        = 2;
    }
}
