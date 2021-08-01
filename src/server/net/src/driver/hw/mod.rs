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

use m3::col::Vec;
use m3::errors::{Code, Error};
use m3::tcu::IRQ;
use m3::vec;

use smoltcp::time::Instant;

extern "C" {
    pub fn axieth_init() -> i32;
    pub fn axieth_send(packet: *const u8, len: usize) -> i32;
    pub fn axieth_recv(buffer: *mut u8, len: usize) -> usize;
    #[allow(dead_code)]
    pub fn axieth_reset() -> i32;
}

pub struct AXIEthDevice;

impl AXIEthDevice {
    pub fn new() -> Result<Self, Error> {
        match unsafe { axieth_init() } {
            0 => Ok(Self {}),
            _ => Err(Error::new(Code::NotFound)),
        }
    }

    pub fn wait_for_irq(&self, timeout_ns: u64) -> Result<(), Error> {
        m3::pexif::wait_irq(
            (1 << IRQ::AXI_FIFO.val) | (1 << IRQ::AXI_ETH.val) | (1 << IRQ::AXI_MAC.val),
            timeout_ns,
        )
    }
}

impl<'a> smoltcp::phy::Device<'a> for AXIEthDevice {
    type RxToken = RxToken;
    type TxToken = TxToken;

    fn capabilities(&self) -> smoltcp::phy::DeviceCapabilities {
        let mut caps = smoltcp::phy::DeviceCapabilities::default();
        caps.max_transmission_unit = 1500;
        // TODO use checksum offloading
        caps.checksum.ipv4 = smoltcp::phy::Checksum::Both;
        caps.checksum.udp = smoltcp::phy::Checksum::Both;
        caps.checksum.tcp = smoltcp::phy::Checksum::Both;
        caps
    }

    fn receive(&'a mut self) -> Option<(Self::RxToken, Self::TxToken)> {
        let mut buffer = vec![0u8; 1500];
        let res = unsafe { axieth_recv((&mut buffer[..]).as_mut_ptr(), buffer.len()) };
        if res == 0 {
            None
        }
        else {
            buffer.resize(res, 0);
            let rx = RxToken { buffer };
            let tx = TxToken {};
            Some((rx, tx))
        }
    }

    fn transmit(&'a mut self) -> Option<Self::TxToken> {
        Some(TxToken {})
    }
}

pub struct RxToken {
    buffer: Vec<u8>,
}

impl smoltcp::phy::RxToken for RxToken {
    fn consume<R, F>(mut self, _timestamp: Instant, f: F) -> smoltcp::Result<R>
    where
        F: FnOnce(&mut [u8]) -> smoltcp::Result<R>,
    {
        f(&mut self.buffer[..])
    }
}

pub struct TxToken;

impl smoltcp::phy::TxToken for TxToken {
    fn consume<R, F>(self, _timestamp: Instant, len: usize, f: F) -> smoltcp::Result<R>
    where
        F: FnOnce(&mut [u8]) -> smoltcp::Result<R>,
    {
        let mut buffer = Vec::<u8>::with_capacity(len);
        // safety: we initialize it below
        unsafe {
            buffer.set_len(len);
        }

        // fill buffer with "to be send" data
        let res = f(&mut buffer)?;

        match unsafe { axieth_send(buffer.as_ptr(), buffer.len()) } {
            0 => Ok(res),
            _ => Err(smoltcp::Error::Exhausted),
        }
    }
}
