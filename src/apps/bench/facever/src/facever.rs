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

use m3::cell::StaticRefCell;
use m3::col::Vec;
use m3::com::{recv_msg, MemGate, RGateArgs, RecvGate, SendGate};
use m3::errors::Error;
use m3::goff;
use m3::kif::Perm;
use m3::mem::{size_of, AlignedBuf, MsgBuf};
use m3::tcu;
use m3::tiles::Activity;
use m3::time::{CycleInstant, Profiler, TimeDuration, TimeInstant};
use m3::util::math::next_log2;
use m3::{env, reply_vmsg};
use m3::{vec, wv_perf};

static BUF: StaticRefCell<AlignedBuf<4096>> = StaticRefCell::new(AlignedBuf::new_zeroed());

fn compute_for(duration: TimeDuration) {
    let end = TimeInstant::now() + duration;
    loop {
        let now = TimeInstant::now();
        if now >= end {
            break;
        }

        Activity::own()
            .sleep_for(end - now)
            .expect("Unable to wait");
    }
}

fn create_reply_gate(ctrl_msg_size: usize) -> Result<RecvGate, Error> {
    let mut reply_gate = RecvGate::new_with(
        RGateArgs::default()
            .order(next_log2(ctrl_msg_size + size_of::<tcu::Header>()))
            .msg_order(next_log2(ctrl_msg_size + size_of::<tcu::Header>())),
    )?;
    reply_gate.activate()?;
    Ok(reply_gate)
}

fn call_and_ack(sgate: &SendGate, ctrl_msg: &MsgBuf, reply_gate: &RecvGate) -> Result<(), Error> {
    let reply = sgate.call(ctrl_msg, reply_gate)?;
    reply_gate.ack_msg(reply)
}

fn client(args: &[&str]) {
    if args.len() != 4 {
        panic!("Usage: {} <ctrl-msg-size> <runs>", args[0]);
    }

    let ctrl_msg_size = args[2]
        .parse::<usize>()
        .expect("Unable to parse control message size");
    let runs = args[3]
        .parse::<u64>()
        .expect("Unable to parse number of runs");

    let mut ctrl_msg = MsgBuf::new();
    ctrl_msg.set(vec![0u8; ctrl_msg_size]);

    let reply_gate = create_reply_gate(ctrl_msg_size).expect("Unable to create reply RecvGate");
    let sgate = SendGate::new_named("req").expect("Unable to create named SendGate req");

    let mut prof = Profiler::default().repeats(runs).warmup(4);

    wv_perf!(
        "faceverification",
        prof.run::<CycleInstant, _>(|| {
            call_and_ack(&sgate, &ctrl_msg, &reply_gate).expect("Request failed");
        })
    );
}

fn frontend(args: &[&str]) {
    if args.len() != 3 {
        panic!("Usage: {} <ctrl-msg-size>", args[0]);
    }

    let ctrl_msg_size = args[2]
        .parse::<usize>()
        .expect("Unable to parse control message size");

    let mut ctrl_msg = MsgBuf::new();
    ctrl_msg.set(vec![0u8; ctrl_msg_size]);

    let fs_sgate = SendGate::new_named("fs").expect("Unable to create named SendGate fs");
    let storage_sgate =
        SendGate::new_named("storage").expect("Unable to create named SendGate storage");
    let mut gpu_rgate =
        RecvGate::new_named("gpures").expect("Unable to create named RecvGate gpures");
    gpu_rgate.activate().expect("Unable to activate RecvGate");

    let reply_gate = create_reply_gate(ctrl_msg_size).expect("Unable to create reply RecvGate");

    let mut req_rgate = RecvGate::new_named("req").expect("Unable to create named RecvGate req");
    req_rgate.activate().expect("Unable to activate RecvGate");
    loop {
        let mut request = recv_msg(&req_rgate).expect("Receiving request failed");

        call_and_ack(&fs_sgate, &ctrl_msg, &reply_gate).expect("fs request failed");

        call_and_ack(&storage_sgate, &ctrl_msg, &reply_gate).expect("storage request failed");

        let mut gpu_res = recv_msg(&gpu_rgate).expect("Receiving GPU result failed");
        reply_vmsg!(gpu_res, 0).expect("Reply to GPU failed");

        request.reply(&ctrl_msg).expect("Reply to client failed");
    }
}

fn fs(args: &[&str]) {
    if args.len() != 4 {
        panic!("Usage: {} <ctrl-msg-size> <compute-millis>", args[0]);
    }

    let ctrl_msg_size = args[2]
        .parse::<usize>()
        .expect("Unable to parse control message size");
    let compute_time = args[3]
        .parse::<u64>()
        .expect("Unable to parse compute time");

    let mut ctrl_msg = MsgBuf::new();
    ctrl_msg.set(vec![0u8; ctrl_msg_size]);

    let mut req_rgate = RecvGate::new_named("fs").expect("Unable to create named RecvGate fs");
    req_rgate.activate().expect("Unable to activate RecvGate");
    loop {
        let mut request = recv_msg(&req_rgate).expect("Receiving request failed");

        compute_for(TimeDuration::from_millis(compute_time));

        request.reply(&ctrl_msg).expect("Reply to client failed");
    }
}

fn gpu(args: &[&str]) {
    if args.len() != 4 {
        panic!("Usage: {} <ctrl-msg-size> <compute-millis>", args[0]);
    }

    let ctrl_msg_size = args[2]
        .parse::<usize>()
        .expect("Unable to parse control message size");
    let compute_time = args[3]
        .parse::<u64>()
        .expect("Unable to parse compute time");

    let mut ctrl_msg = MsgBuf::new();
    ctrl_msg.set(vec![0u8; ctrl_msg_size]);

    let res_sgate = SendGate::new_named("gpures").expect("Unable to create named SendGate gpures");

    let reply_gate = create_reply_gate(ctrl_msg_size).expect("Unable to create reply RecvGate");

    let mut req_rgate = RecvGate::new_named("gpu").expect("Unable to create named RecvGate gpu");
    req_rgate.activate().expect("Unable to activate RecvGate");
    loop {
        let mut request = recv_msg(&req_rgate).expect("Receiving request failed");
        reply_vmsg!(request, 0).expect("Reply to storage failed");

        compute_for(TimeDuration::from_millis(compute_time));

        call_and_ack(&res_sgate, &ctrl_msg, &reply_gate).expect("GPU-result send failed");
    }
}

fn storage(args: &[&str]) {
    if args.len() != 5 {
        panic!(
            "Usage: {} <ctrl-msg-size> <data-size> <compute-millis>",
            args[0]
        );
    }

    let ctrl_msg_size = args[2]
        .parse::<usize>()
        .expect("Unable to parse control message size");
    let data_size = args[3].parse::<usize>().expect("Unable to parse data size");
    let compute_time = args[4]
        .parse::<u64>()
        .expect("Unable to parse compute time");

    let mut ctrl_msg = MsgBuf::new();
    ctrl_msg.set(vec![0u8; ctrl_msg_size]);

    let mem_gate = MemGate::new(data_size, Perm::W).expect("Unable to create memory gate");

    let gpu_sgate = SendGate::new_named("gpu").expect("Unable to create named SendGate gpures");

    let reply_gate = create_reply_gate(ctrl_msg_size).expect("Unable to create reply RecvGate");

    let mut req_rgate =
        RecvGate::new_named("storage").expect("Unable to create named RecvGate storage");
    req_rgate.activate().expect("Unable to activate RecvGate");
    loop {
        let mut request = recv_msg(&req_rgate).expect("Receiving request failed");
        reply_vmsg!(request, 0).expect("Reply to frontend failed");

        compute_for(TimeDuration::from_millis(compute_time));

        let mut count = 0;
        while count < data_size {
            let amount = BUF.borrow().len().min(data_size - count);
            mem_gate
                .write_bytes(BUF.borrow().as_ptr(), amount, count as goff)
                .expect("Writing data failed");
            count += amount;
        }

        call_and_ack(&gpu_sgate, &ctrl_msg, &reply_gate).expect("GPU request failed");
    }
}

#[no_mangle]
pub fn main() -> i32 {
    let args: Vec<&str> = env::args().collect();

    match args[1] {
        "client" => client(&args),
        "frontend" => frontend(&args),
        "fs" => fs(&args),
        "gpu" => gpu(&args),
        "storage" => storage(&args),
        s => panic!("unexpected component {}", s),
    }

    0
}
