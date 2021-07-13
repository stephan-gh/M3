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

#![feature(llvm_asm)]
#![no_std]

extern crate heap;

#[path = "../vmtest/helper.rs"]
mod helper;
#[path = "../vmtest/paging.rs"]
mod paging;

use base::cpu;
use base::log;
use base::math;
use base::mem::MsgBuf;
use base::tcu::{self, EpId, TCU};

const LOG_DEF: bool = true;
const LOG_DETAIL: bool = false;
const LOG_PEXCALLS: bool = false;

const OWN_VPE: u16 = 0xFFFF;

const REP: EpId = tcu::FIRST_USER_EP;
const RPLEPS: EpId = tcu::FIRST_USER_EP + 1;

static RBUF: [u64; 8 * 64] = [0; 8 * 64];

#[no_mangle]
pub extern "C" fn env_run() {
    helper::init("stdareceiver");

    let buf_ord = math::next_log2(RBUF.len());
    let msg_ord = buf_ord - math::next_log2(8);
    let (rbuf_virt, rbuf_phys) = helper::virt_to_phys(RBUF.as_ptr() as usize);
    helper::config_local_ep(REP, |regs| {
        TCU::config_recv(regs, OWN_VPE, rbuf_phys, buf_ord, msg_ord, Some(RPLEPS));
    });

    let mut buf = MsgBuf::new();
    buf.set::<u64>(0);

    log!(crate::LOG_DEF, "Hello World from receiver!");

    for _ in 0..700000 {
        // wait for message
        let rmsg = loop {
            if let Some(m) = helper::fetch_msg(REP, rbuf_virt) {
                break m;
            }
        };
        assert_eq!({ rmsg.header.label }, 0x1234);
        log!(crate::LOG_DETAIL, "got message {}", *rmsg.get_data::<u64>());

        // send reply
        TCU::reply(REP, &buf, TCU::msg_to_offset(rbuf_virt, rmsg)).unwrap();
        buf.set(buf.get::<u64>() + 1);
    }

    // give the other PEs some time
    let begin = cpu::elapsed_cycles();
    while cpu::elapsed_cycles() < begin + 100000 {}

    log!(crate::LOG_DEF, "Shutting down");
    helper::exit(0);
}
