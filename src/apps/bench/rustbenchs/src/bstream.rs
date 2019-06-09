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

use m3::col::{String, ToString};
use m3::com::*;
use m3::profile;
use m3::test;

const MSG_ORD: i32      = 7;
const MSG_SIZE: usize   = 1usize << MSG_ORD;

pub fn run(t: &mut dyn test::Tester) {
    run_test!(t, pingpong_1u64);
    run_test!(t, pingpong_2u64);
    run_test!(t, pingpong_4u64);
    run_test!(t, pingpong_str);
}

fn pingpong_1u64() {
    let reply_gate = RecvGate::def();
    let mut rgate = assert_ok!(RecvGate::new(MSG_ORD, MSG_ORD));
    assert_ok!(rgate.activate());
    let sgate = assert_ok!(SendGate::new_with(SGateArgs::new(&rgate).credits(MSG_SIZE as u64)));

    let mut prof = profile::Profiler::new();

    println!("pingpong with (1 * u64) msgs : {}", prof.run_with_id(|| {
        assert_ok!(send_vmsg!(&sgate, reply_gate, 0u64));

        let mut msg = assert_ok!(recv_msg(&rgate));
        assert_eq!(msg.pop::<u64>(), 0);
        assert_ok!(reply_vmsg!(msg, 0u64));

        let mut reply = assert_ok!(recv_msg(reply_gate));
        assert_eq!(reply.pop::<u64>(), 0);
    }, 0x0));
}

fn pingpong_2u64() {
    let reply_gate = RecvGate::def();
    let mut rgate = assert_ok!(RecvGate::new(MSG_ORD, MSG_ORD));
    assert_ok!(rgate.activate());
    let sgate = assert_ok!(SendGate::new_with(SGateArgs::new(&rgate).credits(MSG_SIZE as u64)));

    let mut prof = profile::Profiler::new();

    println!("pingpong with (2 * u64) msgs : {}", prof.run_with_id(|| {
        assert_ok!(send_vmsg!(&sgate, reply_gate, 23u64, 42u64));

        let mut msg = assert_ok!(recv_msg(&rgate));
        assert_eq!(msg.pop::<u64>(), 23);
        assert_eq!(msg.pop::<u64>(), 42);
        assert_ok!(reply_vmsg!(msg, 5u64, 6u64));

        let mut reply = assert_ok!(recv_msg(reply_gate));
        assert_eq!(reply.pop::<u64>(), 5);
        assert_eq!(reply.pop::<u64>(), 6);
    }, 0x1));
}

fn pingpong_4u64() {
    let reply_gate = RecvGate::def();
    let mut rgate = assert_ok!(RecvGate::new(MSG_ORD, MSG_ORD));
    assert_ok!(rgate.activate());
    let sgate = assert_ok!(SendGate::new_with(SGateArgs::new(&rgate).credits(MSG_SIZE as u64)));

    let mut prof = profile::Profiler::new();

    println!("pingpong with (4 * u64) msgs : {}", prof.run_with_id(|| {
        assert_ok!(send_vmsg!(&sgate, reply_gate, 23u64, 42u64, 10u64, 12u64));

        let mut msg = assert_ok!(recv_msg(&rgate));
        assert_eq!(msg.pop::<u64>(), 23);
        assert_eq!(msg.pop::<u64>(), 42);
        assert_eq!(msg.pop::<u64>(), 10);
        assert_eq!(msg.pop::<u64>(), 12);
        assert_ok!(reply_vmsg!(msg, 5u64, 6u64, 7u64, 8u64));

        let mut reply = assert_ok!(recv_msg(reply_gate));
        assert_eq!(reply.pop::<u64>(), 5);
        assert_eq!(reply.pop::<u64>(), 6);
        assert_eq!(reply.pop::<u64>(), 7);
        assert_eq!(reply.pop::<u64>(), 8);
    }, 0x2));
}

fn pingpong_str() {
    let reply_gate = RecvGate::def();
    let mut rgate = assert_ok!(RecvGate::new(MSG_ORD, MSG_ORD));
    assert_ok!(rgate.activate());
    let sgate = assert_ok!(SendGate::new_with(SGateArgs::new(&rgate).credits(MSG_SIZE as u64)));

    let mut prof = profile::Profiler::new();

    println!("pingpong with (String) msgs  : {}", prof.run_with_id(|| {
        assert_ok!(send_vmsg!(&sgate, reply_gate, "test"));

        let mut msg = assert_ok!(recv_msg(&rgate));
        assert_eq!(msg.pop::<String>(), "test".to_string());
        assert_ok!(reply_vmsg!(msg, "foobar"));

        let mut reply = assert_ok!(recv_msg(reply_gate));
        assert_eq!(reply.pop::<String>(), "foobar".to_string());
    }, 0x3));
}
