/*
 * Copyright (C) 2021, Tendsin Mende <tendsin.mende@mailbox.tu-dresden.de>
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

#![no_std]

#[macro_use]
extern crate m3;

use m3::{
    com::Semaphore,
    env,
    net::{Endpoint, IpAddr, StreamSocketArgs, TcpSocket},
    println,
    session::NetworkManager,
    tcu::TCU,
    vfs::{BufReader, OpenFlags},
};

mod importer;

fn usage() {
    println!("Usage: {} <workload>", env::args().next().unwrap());
    m3::exit(1);
}

#[no_mangle]
pub fn main() -> i32 {
    if env::args().len() < 2 {
        usage();
    }

    let prg_start = TCU::nanotime();

    // Mount fs to load binary data
    m3::vfs::VFS::mount("/", "m3fs", "m3fs").expect("Failed to mount root filesystem on server");

    // open workload file
    let workload_file = env::args().nth(1).unwrap();
    let workload = m3::vfs::VFS::open(workload_file, OpenFlags::R).expect("Could not open file");

    // Connect to server
    let startup_start = TCU::nanotime();
    let nm = NetworkManager::new("net0").expect("Could not connect to network manager");
    let mut socket = TcpSocket::new(
        StreamSocketArgs::new(&nm)
            .send_buffer(64 * 1024)
            .recv_buffer(256 * 1024),
    )
    .expect("Could not create TCP socket");

    // Wait for server to listen
    Semaphore::attach("net").unwrap().down().unwrap();
    socket
        .connect(Endpoint::new(IpAddr::new(127, 0, 0, 1), 1337))
        .expect("Unable to connect to 127.0.0.1:1337");

    let startup = TCU::nanotime() - startup_start;

    // Load workload info for the benchmark
    let mut workload_buffer = BufReader::new(workload);
    let workload_header = importer::WorkloadHeader::load_from_file(&mut workload_buffer);

    let mut send_time: u64 = 0;
    let mut num_send_bytes: u64 = 0;

    let com_start = TCU::nanotime();

    for idx in 0..workload_header.number_of_operations {
        let operation = importer::Package::load_as_bytes(&mut workload_buffer);
        num_send_bytes += operation.len() as u64;
        debug_assert!(importer::Package::from_bytes(&operation).is_ok());

        let this_send = TCU::nanotime();

        socket
            .send(&(operation.len() as u32).to_be_bytes())
            .expect("send failed");
        socket.send(&operation).expect("send failed");
        send_time += TCU::nanotime() - this_send;

        if (idx + 1) % 16 == 0 {
            socket.recv(&mut [0u8; 1]).expect("receive failed");
        }
    }
    let com_time = TCU::nanotime() - com_start;

    let end_msg = b"ENDNOW";
    socket.send(&(end_msg.len() as u32).to_be_bytes()).unwrap();
    socket.send(end_msg).unwrap();

    println!("----YCSB benchmark----");
    println!("Client Side:");
    println!(
        "    Whole benchmark took      {:.4}ns",
        TCU::nanotime() - prg_start
    );
    println!("    Startup took:             {}ns", startup);
    println!(
        "    Avg sender time:   {:.4}ns",
        com_time / workload_header.number_of_operations as u64
    );
    println!(
        "    Avg send time: {:.4}ns",
        send_time / workload_header.number_of_operations as u64
    );
    println!(
        "    Throughput:               {}b/ns",
        num_send_bytes / send_time
    );
    println!("    Send Data                 {}b", num_send_bytes);
    0
}
