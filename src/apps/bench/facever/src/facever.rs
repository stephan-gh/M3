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
use m3::col::{String, ToString, Vec};
use m3::com::{recv_msg, GateIStream, MemGate, RGateArgs, RecvGate, SendGate};
use m3::errors::Error;
use m3::goff;
use m3::kif::Perm;
use m3::mem::{size_of, AlignedBuf, MsgBuf};
use m3::tcu;
use m3::tiles::Activity;
use m3::time::{CycleInstant, Profiler, TimeDuration, TimeInstant};
use m3::util::math::next_log2;
use m3::{env, reply_vmsg};
use m3::{log, vec, wv_perf};

const LOG_MSGS: bool = false;
const LOG_MEM: bool = false;
const LOG_COMP: bool = false;

static BUF: StaticRefCell<AlignedBuf<4096>> = StaticRefCell::new(AlignedBuf::new_zeroed());

fn create_reply_gate(ctrl_msg_size: usize) -> Result<RecvGate, Error> {
    let mut reply_gate = RecvGate::new_with(
        RGateArgs::default()
            .order(next_log2(ctrl_msg_size + size_of::<tcu::Header>()))
            .msg_order(next_log2(ctrl_msg_size + size_of::<tcu::Header>())),
    )?;
    reply_gate.activate()?;
    Ok(reply_gate)
}

struct Node {
    name: String,
    ctrl_msg: MsgBuf,
}

impl Node {
    fn new(name: String, ctrl_msg_size: usize) -> Self {
        let mut ctrl_msg = MsgBuf::new();
        ctrl_msg.set(vec![0u8; ctrl_msg_size]);
        Self { name, ctrl_msg }
    }

    fn compute_for(&self, duration: TimeDuration) {
        log!(
            LOG_COMP,
            "{}: computing for {}ms",
            self.name,
            duration.as_millis()
        );

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

    fn receive_request<'r>(
        &self,
        src: &str,
        rgate: &'r RecvGate,
    ) -> Result<GateIStream<'r>, Error> {
        let request = recv_msg(&rgate)?;
        log!(LOG_MSGS, "{} <- {}", self.name, src);
        Ok(request)
    }

    fn send_reply(&self, dest: &str, request: &mut GateIStream<'_>) -> Result<(), Error> {
        log!(LOG_MSGS, "{} -> {}", self.name, dest);
        request.reply(&self.ctrl_msg)
    }

    fn call_and_ack(
        &self,
        dest: &str,
        sgate: &SendGate,
        reply_gate: &RecvGate,
    ) -> Result<(), Error> {
        log!(LOG_MSGS, "{} -> {}", self.name, dest);
        let reply = sgate.call(&self.ctrl_msg, reply_gate)?;
        log!(LOG_MSGS, "{} <- {}", self.name, dest);
        reply_gate.ack_msg(reply)
    }

    fn write_to(&self, dest: &str, mgate: &MemGate, data_size: usize) -> Result<(), Error> {
        log!(LOG_MEM, "{}: writing to {}", self.name, dest);
        let mut count = 0;
        while count < data_size {
            let amount = BUF.borrow().len().min(data_size - count);
            mgate.write_bytes(BUF.borrow().as_ptr(), amount, count as goff)?;
            count += amount;
        }
        Ok(())
    }
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

    let node = Node::new("client".to_string(), ctrl_msg_size);

    let reply_gate = create_reply_gate(ctrl_msg_size).expect("Unable to create reply RecvGate");
    let sgate = SendGate::new_named("req").expect("Unable to create named SendGate req");

    let mut prof = Profiler::default().repeats(runs).warmup(4);

    wv_perf!(
        "faceverification",
        prof.run::<CycleInstant, _>(|| {
            node.call_and_ack("frontend", &sgate, &reply_gate)
                .expect("Request failed");
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

    let node = Node::new("frontend".to_string(), ctrl_msg_size);

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
        let mut request = node
            .receive_request("client", &req_rgate)
            .expect("Receiving request failed");

        node.call_and_ack("fs", &fs_sgate, &reply_gate)
            .expect("fs request failed");

        node.call_and_ack("storage", &storage_sgate, &reply_gate)
            .expect("storage request failed");

        let mut gpu_res = node
            .receive_request("gpu", &gpu_rgate)
            .expect("Receiving GPU result failed");
        reply_vmsg!(gpu_res, 0).expect("Reply to GPU failed");

        node.send_reply("client", &mut request)
            .expect("Reply to client failed");
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

    let node = Node::new("fs".to_string(), ctrl_msg_size);

    let mut req_rgate = RecvGate::new_named("fs").expect("Unable to create named RecvGate fs");
    req_rgate.activate().expect("Unable to activate RecvGate");
    loop {
        let mut request = node
            .receive_request("frontend", &req_rgate)
            .expect("Receiving request failed");

        node.compute_for(TimeDuration::from_millis(compute_time));

        node.send_reply("frontend", &mut request)
            .expect("Reply to frontend failed");
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

    let node = Node::new("gpu".to_string(), ctrl_msg_size);

    let res_sgate = SendGate::new_named("gpures").expect("Unable to create named SendGate gpures");

    let reply_gate = create_reply_gate(ctrl_msg_size).expect("Unable to create reply RecvGate");

    let mut req_rgate = RecvGate::new_named("gpu").expect("Unable to create named RecvGate gpu");
    req_rgate.activate().expect("Unable to activate RecvGate");
    loop {
        let mut request = node
            .receive_request("storage", &req_rgate)
            .expect("Receiving request failed");
        reply_vmsg!(request, 0).expect("Reply to storage failed");

        node.compute_for(TimeDuration::from_millis(compute_time));

        node.call_and_ack("frontend", &res_sgate, &reply_gate)
            .expect("GPU-result send failed");
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

    let node = Node::new("storage".to_string(), ctrl_msg_size);

    let mem_gate = MemGate::new(data_size, Perm::W).expect("Unable to create memory gate");

    let gpu_sgate = SendGate::new_named("gpu").expect("Unable to create named SendGate gpures");

    let reply_gate = create_reply_gate(ctrl_msg_size).expect("Unable to create reply RecvGate");

    let mut req_rgate =
        RecvGate::new_named("storage").expect("Unable to create named RecvGate storage");
    req_rgate.activate().expect("Unable to activate RecvGate");
    loop {
        let mut request = node
            .receive_request("frontend", &req_rgate)
            .expect("Receiving request failed");
        reply_vmsg!(request, 0).expect("Reply to frontend failed");

        node.compute_for(TimeDuration::from_millis(compute_time));

        node.write_to("gpu", &mem_gate, data_size)
            .expect("Writing data failed");

        node.call_and_ack("gpu", &gpu_sgate, &reply_gate)
            .expect("GPU request failed");
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
