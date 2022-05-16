/*
 * Copyright (C) 2021 Nils Asmussen, Barkhausen Institut
 * Copyright (C) 2021, Tendsin Mende <tendsin.mende@mailbox.tu-dresden.de>
 * Copyright (C) 2017, Georg Kotheimer <georg.kotheimer@mailbox.tu-dresden.de>
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

use m3::cell::{RefCell, StaticRefCell};
use m3::errors::Error;
use m3::rc::Rc;
use m3::vec::Vec;

use smoltcp::time::Instant;

mod defines;
mod e1000;
mod eeprom;

/// Wrapper around the E1000 driver, implementing smols Device trait
pub struct E1000Device {
    dev: Rc<RefCell<e1000::E1000>>,
}

impl E1000Device {
    pub fn new() -> Result<Self, Error> {
        Ok(E1000Device {
            dev: Rc::new(RefCell::new(e1000::E1000::new()?)),
        })
    }

    pub fn needs_poll(&self) -> bool {
        self.dev.borrow().needs_poll()
    }
}

impl<'a> smoltcp::phy::Device<'a> for E1000Device {
    type RxToken = RxToken;
    type TxToken = TxToken;

    fn capabilities(&self) -> smoltcp::phy::DeviceCapabilities {
        let mut caps = smoltcp::phy::DeviceCapabilities::default();
        caps.max_transmission_unit = e1000::E1000::mtu();
        caps.checksum.ipv4 = smoltcp::phy::Checksum::None;
        caps.checksum.udp = smoltcp::phy::Checksum::None;
        caps.checksum.tcp = smoltcp::phy::Checksum::None;
        caps
    }

    fn receive(&'a mut self) -> Option<(Self::RxToken, Self::TxToken)> {
        match self.dev.borrow_mut().receive() {
            Ok(buffer) => {
                let rx = RxToken { buffer };
                let tx = TxToken {
                    device: self.dev.clone(),
                };
                Some((rx, tx))
            },
            Err(_) => None,
        }
    }

    fn transmit(&'a mut self) -> Option<Self::TxToken> {
        Some(TxToken {
            device: self.dev.clone(),
        })
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

pub struct TxToken {
    device: Rc<RefCell<e1000::E1000>>,
}

// use a static and initialized buffer for all packets we send
static SEND_BUF: StaticRefCell<[u8; e1000::E1000::mtu()]> =
    StaticRefCell::new([0u8; e1000::E1000::mtu()]);

impl smoltcp::phy::TxToken for TxToken {
    fn consume<R, F>(self, _timestamp: Instant, len: usize, f: F) -> smoltcp::Result<R>
    where
        F: FnOnce(&mut [u8]) -> smoltcp::Result<R>,
    {
        // fill buffer with "to be send" data
        assert!(len <= SEND_BUF.borrow().len());
        let res = f(&mut SEND_BUF.borrow_mut()[0..len])?;
        match self.device.borrow_mut().send(&SEND_BUF.borrow()[0..len]) {
            true => Ok(res),
            false => Err(smoltcp::Error::Exhausted),
        }
    }
}
