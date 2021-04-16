/*
 * Copyright (C) 2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Copyright (C) 2021, Tendsin Mende <tendsin.mende@mailbox.tu-dresden.de>
 * Copyright (C) 2017, Georg Kotheimer <georg.kotheimer@mailbox.tu-dresden.de>
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

mod dataqueue;

pub mod event;
pub use self::event::{NetEvent, NetEventChannel, NetEventType};

pub mod socket;
pub use self::socket::*;

/// A socket descriptor
pub type Sd = usize;
/// A network port
pub type Port = u16;

pub const MSG_SIZE: usize = 2048;
pub const MSG_ORDER: u32 = 11;
pub const MSG_CREDITS: usize = 4;
pub const MSG_CREDITS_ORDER: u32 = 2;
pub const MSG_BUF_SIZE: usize = MSG_SIZE * MSG_CREDITS;
pub const MSG_BUF_ORDER: u32 = MSG_ORDER + MSG_CREDITS_ORDER;

pub const REPLY_SIZE: usize = 32;
pub const REPLY_ORDER: u32 = 6;
pub const REPLY_BUF_SIZE: usize = REPLY_SIZE * MSG_CREDITS;
pub const REPLY_BUF_ORDER: u32 = REPLY_ORDER + MSG_CREDITS_ORDER;
pub const INBAND_DATA_SIZE: usize = 2048;
pub const INBAND_DATA_CREDITS: usize = 4;
pub const INBAND_DATA_BUF_SIZE: usize = INBAND_DATA_SIZE * INBAND_DATA_CREDITS;
pub const MAX_NETDATA_SIZE: usize = 1024;

/// Represents an internet protocol (IP) address
#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub struct IpAddr(pub u32);

impl IpAddr {
    /// Creates an IP address from given 4 bytes
    pub fn new(v0: u8, v1: u8, v2: u8, v3: u8) -> Self {
        IpAddr(u32::from_be_bytes([v0, v1, v2, v3]))
    }

    /// Creates an unspecified IP address
    pub fn unspecified() -> Self {
        IpAddr::new(0, 0, 0, 0)
    }
}

impl core::fmt::Display for IpAddr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let [a, b, c, d] = self.0.to_be_bytes();
        write!(f, "Ipv4[{}.{}.{}.{}]", a, b, c, d)
    }
}

/// Represents an TCP/UDP endpoint consisting of an IP address and a port
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub struct Endpoint {
    pub addr: IpAddr,
    pub port: Port,
}

impl Endpoint {
    /// Creates a new endpoint for given IP address and port
    pub fn new(addr: IpAddr, port: Port) -> Self {
        Self { addr, port }
    }
}

impl core::fmt::Display for Endpoint {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}:{}", self.addr, self.port)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketType {
    /// TCP socket
    Stream    = 0,
    /// UDP Socket
    Dgram     = 1,
    /// Raw IpSocket
    Raw       = 2,
    Undefined = 3, // Something else
}

impl SocketType {
    pub fn from_usize(ty: usize) -> Self {
        match ty {
            0 => SocketType::Stream,
            1 => SocketType::Dgram,
            2 => SocketType::Raw,
            _ => SocketType::Undefined,
        }
    }
}

/// Represents a media access control address (MAC) address
#[derive(Eq, PartialEq)]
pub struct MAC([u8; 6]);

impl MAC {
    /// Returns the broadcast address
    pub fn broadcast() -> Self {
        MAC([0xff, 0xff, 0xff, 0xff, 0xff, 0xff])
    }

    /// Creates a new MAC address with given bytes
    pub fn new(b0: u8, b1: u8, b2: u8, b3: u8, b4: u8, b5: u8) -> Self {
        MAC([b0, b1, b2, b3, b4, b5])
    }

    /// Returns the MAC address as a u64
    pub fn value(&self) -> u64 {
        return ((self.0[5] as u64) << 40)
            | ((self.0[4] as u64) << 32)
            | ((self.0[3] as u64) << 24)
            | ((self.0[2] as u64) << 16)
            | ((self.0[1] as u64) << 8)
            | ((self.0[0] as u64) << 0);
    }
}

impl core::fmt::Display for MAC {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "MAC[{:x}, {:x}, {:x}, {:x}, {:x}, {:x}]",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5]
        )
    }
}
