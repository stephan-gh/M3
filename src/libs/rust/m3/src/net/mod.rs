/*
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

pub mod socket;
pub use self::socket::*;

pub mod net_channel;
pub use self::net_channel::NetChannel;

use crate::errors::{Code, Error};
use crate::mem;
use crate::serialize::{Marshallable, Sink, Source, Unmarshallable};

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

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub struct IpAddr(pub u32);

impl IpAddr {
    pub fn new(v0: u8, v1: u8, v2: u8, v3: u8) -> Self {
        IpAddr(u32::from_be_bytes([v0, v1, v2, v3]))
    }

    pub fn unspecified() -> Self {
        IpAddr::new(0, 0, 0, 0)
    }
}

impl core::fmt::Display for IpAddr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let [a, b, c, d] = self.0.to_be_bytes();
        write!(f, "Ipv4[{}, {}, {}, {}]", a, b, c, d)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketType {
    ///Tcp socket
    Stream    = 0,
    ///Udp Socket
    Dgram     = 1,
    ///Raw IpSocket
    Raw       = 2,
    Undefined = 3, //Something else
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

#[derive(Debug)]
pub enum SocketState {
    TcpState(crate::net::socket::TcpState),
    UdpState(crate::net::socket::UdpState), //TODO implement?
    RawState, //TODO implement, might not have to since a raw socket has no state?
}

impl Marshallable for SocketState {
    fn marshall(&self, sink: &mut Sink) {
        match self {
            SocketState::TcpState(tcps) => {
                //Is tcp state
                sink.push(&(0 as u64));
                //Push tcp state info
                sink.push(&(*tcps as u64));
            },
            SocketState::UdpState(udps) => {
                //is udpstate
                sink.push(&(1 as u64));
                sink.push(&(*udps as u64));
            },
            SocketState::RawState => {
                //is rawstate
                sink.push(&(2 as u64));
                //No other state info
            },
        }
    }
}

impl Unmarshallable for SocketState {
    fn unmarshall(s: &mut Source) -> Result<Self, Error> {
        let state_type = u64::unmarshall(s)?;

        match state_type {
            0 => {
                let tcp_state = TcpState::from_u64(u64::unmarshall(s)?);
                Ok(SocketState::TcpState(tcp_state))
            },
            1 => {
                let udp_state = UdpState::from_u64(u64::unmarshall(s)?);
                Ok(SocketState::UdpState(udp_state))
            },
            2 => Ok(SocketState::RawState),
            _ => Err(Error::new(Code::WrongSocketType)),
        }
    }
}

///Represents network data that is send over some socket or received.
///
/// Use the `data()` function to try and format the data as any `T`. Use `raw_data()` to receive the bytes.
#[derive(Clone)]
#[repr(C, align(2048))]
pub struct NetData {
    pub sd: i32,
    pub size: u32,
    pub source_addr: IpAddr,
    pub source_port: u16,
    _pad1: u16,
    pub dest_addr: IpAddr,
    pub dest_port: u16,
    _pad2: u16,
    pub data: [u8; MAX_NETDATA_SIZE],
}

impl NetData {
    ///Creates the net data struct from `slice`. Assumes that the slice is not longer than MAX_NETDATA_SIZE.
    pub fn from_slice(
        sd: i32,
        slice: &[u8],
        src_addr: IpAddr,
        src_port: u16,
        dest_addr: IpAddr,
        dest_port: u16,
    ) -> Self {
        let mut data_slice = [0; MAX_NETDATA_SIZE];
        //Copy data into slice
        let copy_size = MAX_NETDATA_SIZE.min(slice.len());
        //Copy the minimum of the slices length and the array length. Therefore, store max `MAX_NETDATA`
        // or, if the slice is shorter, less.
        data_slice[0..(copy_size)].copy_from_slice(&slice[0..(copy_size)]);

        NetData {
            sd,
            size: copy_size as u32,
            data: data_slice,
            source_addr: src_addr,
            source_port: src_port,
            dest_addr,
            dest_port,
            _pad1: 0,
            _pad2: 0,
        }
    }

    pub fn send_size(&self) -> usize {
        6 * mem::size_of::<u32>() + self.size as usize
    }

    pub fn raw_data(&self) -> &[u8] {
        &self.data[0..self.size as usize]
    }
}

impl core::fmt::Display for NetData {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "NetData(sd={}, size={}, srcaddr={:?}:{}, dstaddr={:?}:{}, data=...)",
            self.sd, self.size, self.source_addr, self.source_port, self.dest_addr, self.dest_port
        )
    }
}

pub const MAC_LEN: usize = 6;
#[derive(Eq, PartialEq)]
pub struct MAC([u8; MAC_LEN]);

impl MAC {
    pub fn broadcast() -> Self {
        MAC([0xff, 0xff, 0xff, 0xff, 0xff, 0xff])
    }

    pub fn new(b0: u8, b1: u8, b2: u8, b3: u8, b4: u8, b5: u8) -> Self {
        MAC([b0, b1, b2, b3, b4, b5])
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }

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
