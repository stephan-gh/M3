/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2021 Nils Asmussen, Barkhausen Institut
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

//! The service interface

use super::syscalls::ExchangeArgs;
use crate::errors::Code;
use crate::kif::{CapRngDesc, CapSel};
use crate::serialize::{Deserialize, Serialize};

/// The data part of the delegate/obtain request messages
#[derive(Default, Serialize, Deserialize)]
#[repr(C)]
pub struct ExchangeData {
    pub caps: CapRngDesc,
    pub args: ExchangeArgs,
}

#[derive(Serialize, Deserialize)]
#[repr(C)]
pub enum Request<'s> {
    Open { arg: &'s str },
    DeriveCrt { sessions: usize },
    Obtain { sid: u64, data: ExchangeData },
    Delegate { sid: u64, data: ExchangeData },
    Close { sid: u64 },
    Shutdown,
}

/// The open reply message
#[derive(Serialize, Deserialize)]
#[repr(C)]
pub struct OpenReply {
    pub res: Code,
    pub sid: CapSel,
    pub ident: u64,
}

/// The open reply message
#[derive(Serialize, Deserialize)]
#[repr(C)]
pub struct DeriveCreatorReply {
    pub res: Code,
    pub creator: usize,
    pub sgate_sel: CapSel,
}

/// The delegate/obtain reply message
#[derive(Default, Serialize, Deserialize)]
#[repr(C)]
pub struct ExchangeReply {
    pub res: Code,
    pub data: ExchangeData,
}
