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

use m3::errors::{Code, Error};
use m3::log;
use m3::time::{TimeDuration, TimeInstant};

use pci::Device;

use super::defines::*;
use super::e1000::E1000;

// TODO: Use a sensible value, the current one is chosen arbitrarily
const MAX_WAIT_TIME: TimeDuration = TimeDuration::from_micros(100);

pub struct EEPROM {
    shift: i32,
    done_bit: u32,
}

impl EEPROM {
    pub fn new(device: &Device) -> Result<Self, Error> {
        device.write_reg(REG::EERD.val, EERD::START.bits() as u32)?;

        let t = TimeInstant::now();
        let mut tried_once = false;
        while !tried_once && (TimeInstant::now() - t) < MAX_WAIT_TIME {
            let value: u32 = device.read_reg(REG::EERD.val)?;

            if (value & EERD::DONE_LARGE.bits() as u32) > 0 {
                log!(crate::LOG_NIC, "e1000: detected large EERD");
                return Ok(Self {
                    shift: EERD::SHIFT_LARGE.bits().into(),
                    done_bit: EERD::DONE_LARGE.bits().into(),
                });
            }

            if (value & EERD::DONE_SMALL.bits() as u32) > 0 {
                log!(crate::LOG_NIC, "e1000: detected small EERD");
                return Ok(Self {
                    shift: EERD::SHIFT_SMALL.bits().into(),
                    done_bit: EERD::DONE_SMALL.bits().into(),
                });
            }

            tried_once = true;
        }

        log!(
            crate::LOG_NIC,
            "e1000: timeout while trying to create EEPROM"
        );
        Err(Error::new(Code::Timeout))
    }

    // reads `data` of `len` from the device.
    pub fn read(&self, dev: &E1000, mut address: usize, mut data: &mut [u8]) -> Result<(), Error> {
        assert!((data.len() & 1) == 0);

        let mut len = data.len();
        while len > 0 {
            let word = self.read_word(dev, address)?;
            data[0] = word as u8;
            data[1] = (word >> 8) as u8;
            // move to next word
            data = &mut data[2..];
            address += 1;
            len -= 2;
        }
        Ok(())
    }

    fn read_word(&self, dev: &E1000, address: usize) -> Result<u16, Error> {
        // set address
        dev.write_reg(
            REG::EERD,
            EERD::START.bits() as u32 | (address << self.shift) as u32,
        );

        // Wait for read to complete
        let t = TimeInstant::now();
        let mut done_once = false;
        while (TimeInstant::now() - t) < MAX_WAIT_TIME && !done_once {
            let value = dev.read_reg(REG::EERD);
            done_once = true;
            if (!value & self.done_bit) != 0 {
                // Not read yet, therefore try again
                continue;
            }
            return Ok((value >> 16) as u16);
        }

        Err(Error::new(Code::Timeout))
    }
}
