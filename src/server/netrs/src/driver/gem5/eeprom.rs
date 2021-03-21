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

use m3::errors::{Code, Error};
use m3::log;
use m3::tcu::TCU;

use pci::Device;

use super::defines::*;
use super::e1000::E1000;

const WORD_LEN_LOG2: usize = 1;
// TODO: Use a sensible value, the current one is chosen arbitrarily
const MAX_WAIT_NANOS: u64 = 100000;

pub struct EEPROM {
    shift: i32,
    done_bit: u32,
}

impl EEPROM {
    pub fn new(device: &Device) -> Result<Self, Error> {
        device.write_reg(REG::EERD.val, EERD::START.bits() as u32)?;

        let t = TCU::nanotime();
        let mut tried_once = false;
        while !tried_once && (TCU::nanotime() - t) < MAX_WAIT_NANOS {
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
    // TOD: Currently doing stuff with the ptr of data. Should probably give sub slices of the length of one
    // word tp the read_word fct. Also `len` is not needed since rust slice know their length.
    pub fn read(&self, dev: &E1000, mut address: usize, mut data: &mut [u8]) -> Result<(), Error> {
        assert!((data.len() & ((1 << WORD_LEN_LOG2) - 1)) == 0);

        let num_bytes_to_move = 1 << WORD_LEN_LOG2;
        let mut len = data.len();
        while len > 0 {
            self.read_word(dev, address, data)?;
            // move to next word
            data = &mut data[num_bytes_to_move..];
            address += 1;
            len -= num_bytes_to_move;
        }
        Ok(())
    }

    fn read_word(&self, dev: &E1000, address: usize, data: &mut [u8]) -> Result<(), Error> {
        // cast to 16bit array
        let data_word: &mut [u16] = unsafe { core::mem::transmute::<&mut [u8], &mut [u16]>(data) };

        // set address
        dev.write_reg(
            REG::EERD,
            EERD::START.bits() as u32 | (address << self.shift) as u32,
        );

        // Wait for read to complete
        let t = TCU::nanotime();
        let mut done_once = false;
        while (TCU::nanotime() - t) < MAX_WAIT_NANOS && !done_once {
            let value = dev.read_reg(REG::EERD);
            done_once = true;
            if (!value & self.done_bit) != 0 {
                // Not read yet, therefore try again
                continue;
            }
            // Move word into slice
            data_word[0] = (value >> 16) as u16;
            return Ok(());
        }

        Err(Error::new(Code::Timeout))
    }
}
