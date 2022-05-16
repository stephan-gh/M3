/*
 * Copyright (C) 2021, Tendsin Mende <tendsin.mende@mailbox.tu-dresden.de>
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

/// Conditional include of the driver
#[cfg(target_vendor = "host")]
#[path = "host/mod.rs"]
mod inner;

#[cfg(target_vendor = "gem5")]
#[path = "gem5/mod.rs"]
mod inner;

#[cfg(target_vendor = "hw")]
#[path = "hw/mod.rs"]
mod inner;

pub use inner::*;

use smoltcp::iface::{Context, Interface, SocketHandle};
use smoltcp::socket::AnySocket;
use smoltcp::time::{Duration, Instant};

pub enum DriverInterface<'a> {
    Lo(Interface<'a, smoltcp::phy::Loopback>),
    #[cfg(target_vendor = "gem5")]
    Eth(Interface<'a, E1000Device>),
    #[cfg(target_vendor = "hw")]
    Eth(Interface<'a, AXIEthDevice>),
    #[cfg(target_vendor = "host")]
    Eth(Interface<'a, DevFifo>),
}

impl<'a> DriverInterface<'a> {
    pub fn add_socket<T: AnySocket<'a>>(&mut self, socket: T) -> SocketHandle {
        match self {
            Self::Lo(l) => l.add_socket(socket),
            Self::Eth(e) => e.add_socket(socket),
        }
    }

    pub fn get_socket<T: AnySocket<'a>>(&mut self, handle: SocketHandle) -> &mut T {
        match self {
            Self::Lo(l) => l.get_socket(handle),
            Self::Eth(e) => e.get_socket(handle),
        }
    }

    pub fn get_socket_and_context<T: AnySocket<'a>>(
        &mut self,
        handle: SocketHandle,
    ) -> (&mut T, &mut Context<'a>) {
        match self {
            Self::Lo(l) => l.get_socket_and_context(handle),
            Self::Eth(e) => e.get_socket_and_context(handle),
        }
    }

    pub fn poll(&mut self, timestamp: Instant) -> smoltcp::Result<bool> {
        match self {
            Self::Lo(l) => l.poll(timestamp),
            Self::Eth(e) => e.poll(timestamp),
        }
    }

    pub fn poll_delay(&mut self, timestamp: Instant) -> Option<Duration> {
        match self {
            Self::Lo(l) => l.poll_delay(timestamp),
            Self::Eth(e) => e.poll_delay(timestamp),
        }
    }

    pub fn needs_poll(&self) -> bool {
        match self {
            Self::Lo(_) => false,
            Self::Eth(e) => e.device().needs_poll(),
        }
    }
}
