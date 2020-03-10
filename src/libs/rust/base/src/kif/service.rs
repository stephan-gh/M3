/*
 * Copyright (C) 2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

//! The service interface

use core::fmt;
use kif::syscalls;
use serialize::{Source, Unmarshallable};

/// The maximum size of strings in service calls
pub const MAX_STR_SIZE: usize = 32;

int_enum! {
    /// The service calls
    pub struct Operation : u64 {
        const OPEN          = 0x0;
        const OBTAIN        = 0x1;
        const DELEGATE      = 0x2;
        const CLOSE         = 0x3;
        const SHUTDOWN      = 0x4;
    }
}

/// The open request message
#[repr(C, packed)]
pub struct Open {
    pub opcode: u64,
    pub arglen: u64,
    pub arg: [u8; MAX_STR_SIZE],
}

/// The open reply message
#[repr(C, packed)]
pub struct OpenReply {
    pub res: u64,
    pub sess: u64,
    pub ident: u64,
}

/// The data part of the delegate/obtain request messages
#[repr(C, packed)]
pub struct ExchangeData {
    pub caps: u64,
    pub args: syscalls::ExchangeArgs,
}

/// The delegate/obtain request message
#[repr(C, packed)]
pub struct Exchange {
    pub opcode: u64,
    pub sess: u64,
    pub data: ExchangeData,
}

/// The delegate/obtain reply message
#[repr(C, packed)]
pub struct ExchangeReply {
    pub res: u64,
    pub data: ExchangeData,
}

/// The close request message
#[repr(C, packed)]
pub struct Close {
    pub opcode: u64,
    pub sess: u64,
}

/// The shutdown request message
#[repr(C, packed)]
pub struct Shutdown {
    pub opcode: u64,
}

impl fmt::Debug for ExchangeData {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "ExchangeData[")?;
        for i in 0..self.args.count() {
            let arg = self.args.ival(i);
            write!(f, "{}", arg)?;
            if i + 1 < self.args.count() {
                write!(f, ", ")?;
            }
        }
        write!(f, "]")
    }
}

impl Unmarshallable for ExchangeData {
    fn unmarshall(s: &mut dyn Source) -> Self {
        let mut res = ExchangeData {
            caps: s.pop_word(),
            args: syscalls::ExchangeArgs::default(),
        };
        res.args.set_count(s.pop_word() as usize);
        for i in 0..syscalls::MAX_EXCHG_ARGS {
            res.args.set_ival(i, s.pop_word());
        }
        res
    }
}
