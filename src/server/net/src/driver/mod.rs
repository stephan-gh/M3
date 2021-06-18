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
#[cfg(target_os = "linux")]
#[path = "host/mod.rs"]
mod inner;

#[cfg(target_os = "none")]
#[path = "gem5/mod.rs"]
mod inner;

pub use inner::*;

use smoltcp::iface::Interface;
use smoltcp::socket::SocketSet;
use smoltcp::time::{Duration, Instant};

pub enum DriverInterface<'a> {
    Lo(Interface<'a, smoltcp::phy::Loopback>),
    #[cfg(target_os = "none")]
    Eth(Interface<'a, E1000Device>),
    #[cfg(target_os = "linux")]
    Eth(Interface<'a, DevFifo>),
}

impl<'a> DriverInterface<'a> {
    pub fn poll(
        &mut self,
        sockets: &mut SocketSet<'_>,
        timestamp: Instant,
    ) -> smoltcp::Result<bool> {
        match self {
            Self::Lo(l) => l.poll(sockets, timestamp),
            Self::Eth(e) => e.poll(sockets, timestamp),
        }
    }

    pub fn poll_delay(&self, sockets: &SocketSet<'_>, timestamp: Instant) -> Option<Duration> {
        match self {
            Self::Lo(l) => l.poll_delay(sockets, timestamp),
            Self::Eth(e) => e.poll_delay(sockets, timestamp),
        }
    }
}
