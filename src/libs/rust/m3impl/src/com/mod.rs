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

//! Contains communication abstractions
//!
//! Communication on M³ is performed via the trusted communication unit (TCU) and comes in two
//! primary flavors: message passing and DMA-like memory access. The communication abstractions can
//! be devided in two layers: lower-level primitives that work with the TCU and higher-level
//! primitives and build on top of them.
//!
//! # Endpoints and gates
//!
//! The lower-level primitives are [`endpoints`](`EP`) and [`gates`](`gate::Gate`). Each TCU-based
//! communication channel is represented by *endpoints* (EPs) in the TCU. A message-passing channel
//! consists of a send EP and a receive EP, whereas a memory-channel consists of a single memory EP.
//! A *gate* is the software abstraction that comes in three variants, corresponding to the endpoint
//! types: [`SendGate`], [`RecvGate`], and [`MemGate`]. All gates therefore use a specific endpoint
//! for the communication and need to be *activated* before they can be used. The activation of a
//! gate allocates an endpoint (if required) and configures the endpoint for the gate.
//!
//! # Streams and channels
//!
//! The higher-level primitives are streams and channels. [`GateOStream`] allows to marshall data
//! types into a message, whereas [`GateIStream`] allows to unmarshall a message into data types.
//! Both work in combination with [`SendGate`]s and [`RecvGate`]s, respectively. A
//! [`channel`](`chan::sync_channel`) provides a synchronous uni-directional communication channel
//! based on gates.

#[macro_use]
mod stream;

pub mod chan;
mod ep;
mod epmng;
mod gate;
mod mgate;
pub mod opcodes;
mod rbufs;
mod rgate;
mod sem;
mod sgate;

pub use self::ep::{EPArgs, EP};
pub use self::epmng::EpMng;
pub use self::gate::{Gate, GateCap, LazyGate};
pub use self::mgate::{MGateArgs, MemGate, Perm};
pub use self::rbufs::RecvBuf;
pub use self::rgate::{RGateArgs, ReceivingGate, RecvCap, RecvGate};
pub use self::sem::Semaphore;
pub use self::sgate::{SGateArgs, SendCap, SendGate};
pub use self::stream::*;
pub use base::msgqueue::{MsgQueue, MsgSender};

pub(crate) fn pre_init() {
    rgate::pre_init();
}

pub(crate) fn init() {
    rbufs::init();
}
