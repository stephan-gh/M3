/*
 * Copyright (C) 2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

use base::envdata;
use base::pexif;

use crate::arch::{env, pexcalls};
use crate::errors::Error;
use crate::tcu;

pub struct TCUIf {}

impl TCUIf {
    #[inline(always)]
    pub fn sleep() -> Result<(), Error> {
        Self::sleep_for(0)
    }

    #[inline(always)]
    pub fn sleep_for(nanos: u64) -> Result<(), Error> {
        if envdata::get().platform == envdata::Platform::GEM5.val {
            if env::get().shared() || nanos != 0 {
                pexcalls::call2(
                    pexif::Operation::SLEEP,
                    nanos as usize,
                    tcu::INVALID_EP as usize,
                )
                .map(|_| ())
            }
            else {
                tcu::TCU::wait_for_msg(tcu::INVALID_EP)
            }
        }
        else {
            Ok(())
        }
    }

    pub fn wait_for_msg(ep: tcu::EpId) -> Result<(), Error> {
        if envdata::get().platform == envdata::Platform::GEM5.val {
            if env::get().shared() {
                pexcalls::call2(pexif::Operation::SLEEP, 0, ep as usize).map(|_| ())
            }
            else {
                tcu::TCU::wait_for_msg(ep)
            }
        }
        else {
            Ok(())
        }
    }

    #[inline(always)]
    pub fn switch_vpe() -> Result<(), Error> {
        pexcalls::call1(pexif::Operation::YIELD, 0).map(|_| ())
    }

    pub fn flush_invalidate() -> Result<(), Error> {
        pexcalls::call1(pexif::Operation::FLUSH_INV, 0).map(|_| ())
    }

    #[inline(always)]
    pub fn noop() -> Result<(), Error> {
        pexcalls::call1(pexif::Operation::NOOP, 0).map(|_| ())
    }
}
