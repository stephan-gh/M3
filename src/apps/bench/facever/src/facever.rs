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

use m3::col::{String, ToString, Vec};
use m3::com::{recv_msg, GateIStream, MemGate, RGateArgs, RecvGate, SendGate};
use m3::errors::Error;
use m3::io::LogFlags;
use m3::kif::Perm;
use m3::mem::{size_of, MsgBuf};
use m3::time::{CycleDuration, CycleInstant, Duration, Profiler};
use m3::util::math::next_log2;
use m3::{env, reply_vmsg};
use m3::{log, println, vec};
use m3::{tcu, wv_perf};

fn create_reply_gate(ctrl_msg_size: usize) -> Result<RecvGate, Error> {
    RecvGate::new_with(
        RGateArgs::default()
            .order(next_log2(ctrl_msg_size + size_of::<tcu::Header>()))
            .msg_order(next_log2(ctrl_msg_size + size_of::<tcu::Header>())),
    )
}

struct Node {
    name: String,
    ctrl_msg: MsgBuf,
    data_buf: Vec<u8>,
}

impl Node {
    fn new(name: String, ctrl_msg_size: usize, data_size: usize) -> Self {
        let mut ctrl_msg = MsgBuf::new();
        ctrl_msg.set(vec![0u8; ctrl_msg_size]);
        let data_buf = vec![0u8; data_size];
        Self {
            name,
            ctrl_msg,
            data_buf,
        }
    }

    fn compute_for(&self, duration: CycleDuration) {
        log!(
            LogFlags::Debug,
            "{}: computing for {:?}",
            self.name,
            duration
        );

        let end = CycleInstant::now().as_cycles() + duration.as_raw();
        while CycleInstant::now().as_cycles() < end {}
    }

    fn receive_request<'r>(
        &self,
        src: &str,
        rgate: &'r RecvGate,
    ) -> Result<GateIStream<'r>, Error> {
        let request = recv_msg(rgate)?;
        log!(LogFlags::Debug, "{} <- {}", self.name, src);
        Ok(request)
    }

    fn send_reply(&self, dest: &str, request: &mut GateIStream<'_>) -> Result<(), Error> {
        log!(LogFlags::Debug, "{} -> {}", self.name, dest);
        request.reply(&self.ctrl_msg)
    }

    fn call_and_ack(
        &self,
        dest: &str,
        sgate: &SendGate,
        reply_gate: &RecvGate,
    ) -> Result<(), Error> {
        log!(LogFlags::Debug, "{} -> {}", self.name, dest);
        let reply = sgate.call(&self.ctrl_msg, reply_gate)?;
        log!(LogFlags::Debug, "{} <- {}", self.name, dest);
        reply_gate.ack_msg(reply)
    }

    fn write_to(&self, dest: &str, mgate: &MemGate, data_size: usize) -> Result<(), Error> {
        log!(LogFlags::Debug, "{}: writing to {}", self.name, dest);
        mgate.write(&self.data_buf[0..data_size], 0)
    }
}

fn client(args: &[&str]) {
    if args.len() != 5 {
        panic!("Usage: {} <ctrl-msg-size> <all-compute> <runs>", args[0]);
    }

    let ctrl_msg_size = args[2]
        .parse::<usize>()
        .expect("Unable to parse control message size");
    let compute_time = args[3]
        .parse::<u64>()
        .expect("Unable to parse compute time");
    let runs = args[4]
        .parse::<u64>()
        .expect("Unable to parse number of runs");

    let node = Node::new("client".to_string(), ctrl_msg_size, 0);

    let reply_gate = create_reply_gate(ctrl_msg_size).expect("Unable to create reply RecvGate");
    let sgate = SendGate::new_named("req").expect("Unable to create named SendGate req");

    let prof = Profiler::default().warmup(1).repeats(runs - 1);
    let res = prof.run::<CycleInstant, _>(|| {
        let start = CycleInstant::now();

        node.call_and_ack("frontend", &sgate, &reply_gate)
            .expect("Request failed");

        let duration = CycleInstant::now().duration_since(start);
        let duration = if env::get().platform() == env::Platform::Hw {
            // compensate for running on a 100MHz core (in contrast to the computing computes that run
            // on a 80MHz core).
            ((duration.as_raw() as f64) * 0.8) as u64
        }
        else {
            duration.as_raw()
        };
        println!("total: {}", duration);
        println!("compute: {}", compute_time);
    });

    wv_perf!("Face verification", res);
}

fn frontend(args: &[&str]) {
    if args.len() != 3 {
        panic!("Usage: {} <ctrl-msg-size>", args[0]);
    }

    let ctrl_msg_size = args[2]
        .parse::<usize>()
        .expect("Unable to parse control message size");

    let node = Node::new("frontend".to_string(), ctrl_msg_size, 0);

    let gpu_rgate = RecvGate::new_named("gpures").expect("Unable to create named RecvGate gpures");

    let reply_gate = create_reply_gate(ctrl_msg_size).expect("Unable to create reply RecvGate");

    let req_rgate = RecvGate::new_named("req").expect("Unable to create named RecvGate req");

    let fs_sgate = SendGate::new_named("fs").expect("Unable to create named SendGate fs");
    let storage_sgate =
        SendGate::new_named("storage").expect("Unable to create named SendGate storage");

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

    let node = Node::new("fs".to_string(), ctrl_msg_size, 0);

    let req_rgate = RecvGate::new_named("fs").expect("Unable to create named RecvGate fs");
    loop {
        let mut request = node
            .receive_request("frontend", &req_rgate)
            .expect("Receiving request failed");

        node.compute_for(CycleDuration::from_raw(compute_time));

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

    let node = Node::new("gpu".to_string(), ctrl_msg_size, 0);

    let reply_gate = create_reply_gate(ctrl_msg_size).expect("Unable to create reply RecvGate");

    let req_rgate = RecvGate::new_named("gpu").expect("Unable to create named RecvGate gpu");

    let res_sgate = SendGate::new_named("gpures").expect("Unable to create named SendGate gpures");

    loop {
        let mut request = node
            .receive_request("storage", &req_rgate)
            .expect("Receiving request failed");
        reply_vmsg!(request, 0).expect("Reply to storage failed");

        node.compute_for(CycleDuration::from_raw(compute_time));

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

    let node = Node::new("storage".to_string(), ctrl_msg_size, data_size);

    let mem_gate = if data_size > 0 {
        Some(MemGate::new(data_size, Perm::W).expect("Unable to create memory gate"))
    }
    else {
        None
    };

    let reply_gate = create_reply_gate(ctrl_msg_size).expect("Unable to create reply RecvGate");

    let req_rgate =
        RecvGate::new_named("storage").expect("Unable to create named RecvGate storage");

    let gpu_sgate = SendGate::new_named("gpu").expect("Unable to create named SendGate gpures");

    loop {
        let mut request = node
            .receive_request("frontend", &req_rgate)
            .expect("Receiving request failed");
        reply_vmsg!(request, 0).expect("Reply to frontend failed");

        node.compute_for(CycleDuration::from_raw(compute_time));

        if let Some(ref mg) = mem_gate {
            let start = CycleInstant::now();
            node.write_to("gpu", mg, data_size)
                .expect("Writing data failed");
            let duration = CycleInstant::now().duration_since(start);
            println!("xfer: {:?}", duration);
        }

        node.call_and_ack("gpu", &gpu_sgate, &reply_gate)
            .expect("GPU request failed");
    }
}

#[no_mangle]
pub fn main() -> Result<(), Error> {
    let args: Vec<&str> = env::args().collect();

    match args[1] {
        "client" => client(&args),
        "frontend" => frontend(&args),
        "fs" => fs(&args),
        "gpu" => gpu(&args),
        "storage" => storage(&args),
        s => panic!("unexpected component {}", s),
    }

    Ok(())
}
