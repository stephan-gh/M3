/*
 * Copyright (C) 2023-2024, Stephan Gerhold <stephan@gerhold.net>
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

#![no_std]
#![no_main]
#![feature(asm_const)]

use riscv_rt::entry;

use base::io::log::LogColor;
use base::io::{log, LogFlags};
use base::tcu::TCU;
use base::{env, log, machine};
#[allow(unused_imports)]
use lang as _;
use rot::cshake::kmac;
use rot::Secret;

const UDS_SIZE: usize = 256 / u8::BITS as usize;
static UDS: Secret<[u8; UDS_SIZE]> = Secret::new_zeroed(); // Dummy UDS (all zeroes)

const KMAC_KEY_PAD_UDS: &str = "UDS";

mod asm;

#[no_mangle]
pub extern "C" fn exit(_code: i32) -> ! {
    log!(LogFlags::Info, "Shutting down");
    machine::shutdown();
}

#[entry]
fn main() -> ! {
    log::init(env::boot().tile_id(), "brom", LogColor::BrightRed);
    log!(LogFlags::RoTBoot, "Hello World!");

    {
        // Load RoT configuration to the reserved region at end of memory
        let reservation = unsafe { rot::cfg_reservation() };
        TCU::read_slice(rot::FLASH_EP, reservation, 0).expect("Failed to read RoT config");
    }
    let cfg = unsafe { rot::BromLayerCfg::get() };

    // Load binary for next layer
    let next = unsafe { rot::load_bin(rot::BROM_NEXT_ADDR, &cfg.data.next_layer) };

    // Prepare KMAC key with copied UDS
    let mut kmac_uds = Secret::new_zeroed();
    let off = kmac::write_partial_key(&mut kmac_uds.secret, KMAC_KEY_PAD_UDS.as_bytes(), UDS_SIZE);
    kmac_uds.secret[off..off + UDS_SIZE].copy_from_slice(&UDS.secret);

    // Derive CDI for next layer
    let mut ctx = rot::LayerCtx::new(rot::BROM_NEXT_ADDR, rot::BromCtx {
        kmac_cdi: Secret::new_zeroed(),
    });
    rot::derive_cdi(&kmac_uds, next, &mut ctx.data.kmac_cdi);

    // TODO: Lock UDS access (there is no actual UDS at the moment)
    unsafe { ctx.switch() }
}
