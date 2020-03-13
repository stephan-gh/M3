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

use m3::col::String;
use m3::com::{recv_msg, RecvGate, SGateArgs, SendGate};
use m3::profile;
use m3::test;

const MSG_ORD: u32 = 8;

pub fn run(t: &mut dyn test::WvTester) {
    wv_run_test!(t, pingpong_1u64);
    wv_run_test!(t, pingpong_2u64);
    wv_run_test!(t, pingpong_4u64);
    wv_run_test!(t, pingpong_str);
    wv_run_test!(t, pingpong_strslice);
}

fn pingpong_1u64() {
    let reply_gate = RecvGate::def();
    let mut rgate = wv_assert_ok!(RecvGate::new(MSG_ORD, MSG_ORD));
    wv_assert_ok!(rgate.activate());
    let sgate = wv_assert_ok!(SendGate::new_with(SGateArgs::new(&rgate).credits(1)));

    let mut prof = profile::Profiler::default();

    wv_perf!(
        "pingpong with (1 * u64) msgs",
        prof.run_with_id(
            || {
                wv_assert_ok!(send_vmsg!(&sgate, reply_gate, 0u64));

                let mut msg = wv_assert_ok!(recv_msg(&rgate));
                wv_assert_eq!(msg.pop::<u64>(), Ok(0));
                wv_assert_ok!(reply_vmsg!(msg, 0u64));

                let mut reply = wv_assert_ok!(recv_msg(reply_gate));
                wv_assert_eq!(reply.pop::<u64>(), Ok(0));
            },
            0x0
        )
    );
}

fn pingpong_2u64() {
    let reply_gate = RecvGate::def();
    let mut rgate = wv_assert_ok!(RecvGate::new(MSG_ORD, MSG_ORD));
    wv_assert_ok!(rgate.activate());
    let sgate = wv_assert_ok!(SendGate::new_with(SGateArgs::new(&rgate).credits(1)));

    let mut prof = profile::Profiler::default();

    wv_perf!(
        "pingpong with (2 * u64) msgs",
        prof.run_with_id(
            || {
                wv_assert_ok!(send_vmsg!(&sgate, reply_gate, 23u64, 42u64));

                let mut msg = wv_assert_ok!(recv_msg(&rgate));
                wv_assert_eq!(msg.pop::<u64>(), Ok(23));
                wv_assert_eq!(msg.pop::<u64>(), Ok(42));
                wv_assert_ok!(reply_vmsg!(msg, 5u64, 6u64));

                let mut reply = wv_assert_ok!(recv_msg(reply_gate));
                wv_assert_eq!(reply.pop::<u64>(), Ok(5));
                wv_assert_eq!(reply.pop::<u64>(), Ok(6));
            },
            0x1
        )
    );
}

fn pingpong_4u64() {
    let reply_gate = RecvGate::def();
    let mut rgate = wv_assert_ok!(RecvGate::new(MSG_ORD, MSG_ORD));
    wv_assert_ok!(rgate.activate());
    let sgate = wv_assert_ok!(SendGate::new_with(SGateArgs::new(&rgate).credits(1)));

    let mut prof = profile::Profiler::default();

    wv_perf!(
        "pingpong with (4 * u64) msgs",
        prof.run_with_id(
            || {
                wv_assert_ok!(send_vmsg!(&sgate, reply_gate, 23u64, 42u64, 10u64, 12u64));

                let mut msg = wv_assert_ok!(recv_msg(&rgate));
                wv_assert_eq!(msg.pop::<u64>(), Ok(23));
                wv_assert_eq!(msg.pop::<u64>(), Ok(42));
                wv_assert_eq!(msg.pop::<u64>(), Ok(10));
                wv_assert_eq!(msg.pop::<u64>(), Ok(12));
                wv_assert_ok!(reply_vmsg!(msg, 5u64, 6u64, 7u64, 8u64));

                let mut reply = wv_assert_ok!(recv_msg(reply_gate));
                wv_assert_eq!(reply.pop::<u64>(), Ok(5));
                wv_assert_eq!(reply.pop::<u64>(), Ok(6));
                wv_assert_eq!(reply.pop::<u64>(), Ok(7));
                wv_assert_eq!(reply.pop::<u64>(), Ok(8));
            },
            0x2
        )
    );
}

fn pingpong_str() {
    let reply_gate = RecvGate::def();
    let mut rgate = wv_assert_ok!(RecvGate::new(MSG_ORD, MSG_ORD));
    wv_assert_ok!(rgate.activate());
    let sgate = wv_assert_ok!(SendGate::new_with(SGateArgs::new(&rgate).credits(1)));

    let mut prof = profile::Profiler::default();

    wv_perf!(
        "pingpong with (String) msgs",
        prof.run_with_id(
            || {
                wv_assert_ok!(send_vmsg!(&sgate, reply_gate, "test"));

                let mut msg = wv_assert_ok!(recv_msg(&rgate));
                wv_assert_eq!(msg.pop::<String>().unwrap().len(), 4);
                wv_assert_ok!(reply_vmsg!(msg, "foobar"));

                let mut reply = wv_assert_ok!(recv_msg(reply_gate));
                wv_assert_eq!(reply.pop::<String>().unwrap().len(), 6);
            },
            0x3
        )
    );
}

fn pingpong_strslice() {
    let reply_gate = RecvGate::def();
    let mut rgate = wv_assert_ok!(RecvGate::new(MSG_ORD, MSG_ORD));
    wv_assert_ok!(rgate.activate());
    let sgate = wv_assert_ok!(SendGate::new_with(SGateArgs::new(&rgate).credits(1)));

    let mut prof = profile::Profiler::default();

    wv_perf!(
        "pingpong with (&str) msgs",
        prof.run_with_id(
            || {
                wv_assert_ok!(send_vmsg!(&sgate, reply_gate, "test"));

                let mut msg = wv_assert_ok!(recv_msg(&rgate));
                wv_assert_eq!(msg.pop::<&str>().unwrap().len(), 4);
                wv_assert_ok!(reply_vmsg!(msg, "foobar"));

                let mut reply = wv_assert_ok!(recv_msg(reply_gate));
                wv_assert_eq!(reply.pop::<&str>().unwrap().len(), 6);
            },
            0x4
        )
    );
}
