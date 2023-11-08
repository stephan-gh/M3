/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2021 Nils Asmussen, Barkhausen Institut
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

use m3::cap::Selector;
use m3::cell::StaticRefCell;
use m3::com::{recv_msg, RecvCap, RecvGate, SGateArgs, SendGate};
use m3::errors::{Code, Error};
use m3::mem::{size_of, AlignedBuf};
use m3::tcu;
use m3::test::{DefaultWvTester, WvTester};
use m3::tiles::{Activity, ActivityArgs, ChildActivity, RunningActivity, Tile};
use m3::time::{CycleInstant, Profiler};
use m3::util::math::next_log2;
use m3::{format, wv_assert_eq, wv_assert_ok, wv_perf};

static BUF: StaticRefCell<AlignedBuf<8192>> = StaticRefCell::new(AlignedBuf::new_zeroed());

#[no_mangle]
pub fn main() -> Result<(), Error> {
    let mut t = DefaultWvTester::default();

    let tile = wv_assert_ok!(Tile::get("compat"));
    let mut act = wv_assert_ok!(ChildActivity::new_with(tile, ActivityArgs::new("sender")));

    const MSG_ORD: u32 = next_log2(2048);
    const RUNS: u64 = 1000;
    const WARMUP: u64 = 100;
    const MAX_MSG_SIZE: usize = 2048 - size_of::<tcu::Header>();

    let rgate = wv_assert_ok!(RecvCap::new(MSG_ORD, MSG_ORD));

    wv_assert_ok!(act.delegate_obj(rgate.sel()));

    let mut dst = act.data_sink();
    dst.push(rgate.sel());

    let act = wv_assert_ok!(act.run(|| {
        let rgate_sel: Selector = Activity::own().data_source().pop().unwrap();
        let rgate = RecvGate::new_bind(rgate_sel).unwrap();

        for i in 0..=MSG_ORD {
            let size = (1 << i).min(MAX_MSG_SIZE);
            for _ in 0..RUNS + WARMUP {
                let mut msg = wv_assert_ok!(recv_msg(&rgate));
                wv_assert_ok!(msg.reply_aligned(BUF.borrow().as_ptr(), size));
            }
        }
        Ok(())
    }));

    let sgate = wv_assert_ok!(SendGate::new_with(SGateArgs::new(&rgate).credits(1)));
    let reply_gate = wv_assert_ok!(RecvGate::new(MSG_ORD, MSG_ORD));

    for i in 0..=MSG_ORD {
        let prof = Profiler::default().repeats(RUNS).warmup(WARMUP);

        let size = (1 << i).min(MAX_MSG_SIZE);

        wv_perf!(
            format!("pingpong with {}b msgs", size),
            prof.run::<CycleInstant, _>(|| {
                wv_assert_ok!(sgate.send_aligned(BUF.borrow().as_ptr(), size, &reply_gate));
                let msg = wv_assert_ok!(reply_gate.receive(Some(&sgate)));
                wv_assert_ok!(reply_gate.ack_msg(msg));
            })
        );
    }

    wv_assert_eq!(t, act.wait(), Ok(Code::Success));

    Ok(())
}
