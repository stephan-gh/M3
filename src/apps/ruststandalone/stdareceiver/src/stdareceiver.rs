/*
 * Copyright (C) 2021-2022 Nils Asmussen, Barkhausen Institut
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

#![no_std]

#[allow(unused_extern_crates)]
extern crate heap;

#[path = "../../vmtest/src/helper.rs"]
mod helper;
#[path = "../../vmtest/src/paging.rs"]
mod paging;

use base::cpu::{CPUOps, CPU};
use base::env;
use base::io::LogFlags;
use base::log;
use base::mem::{MsgBuf, VirtAddr};
use base::tcu::{self, EpId, TCU};
use base::util::math;

const OWN_ACT: u16 = 0xFFFF;
const CREDITS: usize = 4;
const CLIENTS: usize = 8;
const MSG_SIZE: usize = 64;

const REP: EpId = tcu::FIRST_USER_EP;
const RPLEPS: EpId = tcu::FIRST_USER_EP + 1;

static RBUF: [u64; CREDITS * CLIENTS * MSG_SIZE] = [0; CREDITS * CLIENTS * MSG_SIZE];

#[no_mangle]
pub extern "C" fn env_run() {
    helper::init("stdareceiver");

    let buf_ord = math::next_log2(RBUF.len());
    let msg_ord = buf_ord - math::next_log2(CLIENTS * CREDITS);
    let (rbuf_virt, rbuf_phys) = helper::virt_to_phys(VirtAddr::from(RBUF.as_ptr()));
    helper::config_local_ep(REP, |regs| {
        TCU::config_recv(regs, OWN_ACT, rbuf_phys, buf_ord, msg_ord, Some(RPLEPS));
    });

    let mut buf = MsgBuf::new();
    buf.set::<u64>(0);

    log!(LogFlags::Info, "Hello World from receiver!");

    let sends = match env::boot().platform {
        env::Platform::Hw => 100000,
        env::Platform::Gem5 => 100,
    };

    for recv in 0..sends * 7 {
        // wait for message
        let rmsg = loop {
            if let Some(m) = helper::fetch_msg(REP, rbuf_virt) {
                break m;
            }
        };
        assert_eq!(rmsg.header.label(), 0x1234);
        log!(LogFlags::Debug, "got message {}", rmsg.as_words()[0]);

        // send reply
        TCU::reply(REP, &buf, TCU::msg_to_offset(rbuf_virt, rmsg)).unwrap();
        buf.set(buf.get::<u64>() + 1);

        if recv % 1000 == 0 {
            log!(LogFlags::Info, "Received {} messages", recv);
        }
    }

    // give the other tiles some time
    let begin = CPU::elapsed_cycles();
    while CPU::elapsed_cycles() < begin + 100000 {}

    log!(LogFlags::Info, "Shutting down");
    helper::exit(0);
}
