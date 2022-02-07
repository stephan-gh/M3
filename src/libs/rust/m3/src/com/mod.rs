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

//! Contains communication abstractions.

#[macro_use]
mod stream;

mod ep;
mod epmng;
mod gate;
mod mgate;
mod rbufs;
mod rgate;
mod sem;
mod sgate;

pub use self::ep::{EPArgs, EP};
pub use self::epmng::EpMng;
pub use self::mgate::{MGateArgs, MemGate, Perm};
pub use self::rbufs::{alloc_rbuf, free_rbuf, RecvBuf};
pub use self::rgate::{RGateArgs, RecvGate};
pub use self::sem::Semaphore;
pub use self::sgate::{SGateArgs, SendGate};
pub use self::stream::*;
pub use base::msgqueue::{MsgQueue, MsgSender};

pub(crate) fn pre_init() {
    rgate::pre_init();
}

pub(crate) fn init() {
    rbufs::init();
}
