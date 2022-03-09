/*
 * Copyright (C) 2021, Tendsin Mende <tendsin.mende@mailbox.tu-dresden.de>
 * Copyright (C) 2021 Nils Asmussen, Barkhausen Institut
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
    col::Vec,
    com::Semaphore,
    env,
    net::{DgramSocketArgs, Endpoint, IpAddr, Port, StreamSocketArgs, TcpSocket, UdpSocket},
    println,
    session::NetworkManager,
    vfs::{BufReader, OpenFlags},
};

mod importer;

const VERBOSE: bool = false;

fn usage() {
    let name = env::args().next().unwrap();
    println!("Usage: {} tcp <ip> <port> <workload> <repeats>", name);
    println!("Usage: {} udp <port>", name);
    m3::exit(1);
}

fn udp_receiver(nm: &NetworkManager, port: Port) {
    let mut socket = UdpSocket::new(
        DgramSocketArgs::new(nm)
            .send_buffer(2, 1024)
            .recv_buffer(128, 768 * 1024),
    )
    .expect("Could not create TCP socket");

    socket.bind(port).expect("Could not bind socket");

    let mut buf = vec![0u8; 1024];
    loop {
        let amount = socket.recv(&mut buf).expect("Receive failed");
        if VERBOSE {
            println!("Received {} bytes.", amount);
        }
    }
}

fn tcp_sender(nm: &NetworkManager, ip: IpAddr, port: Port, wl: &str, repeats: u32) {
    // Mount fs to load binary data
    m3::vfs::VFS::mount("/", "m3fs", "m3fs").expect("Failed to mount root filesystem on server");

    // Connect to server
    let mut socket = TcpSocket::new(
        StreamSocketArgs::new(nm)
            .send_buffer(64 * 1024)
            .recv_buffer(256 * 1024),
    )
    .expect("Could not create TCP socket");

    // Wait for server to listen
    Semaphore::attach("net").unwrap().down().unwrap();
    socket
        .connect(Endpoint::new(ip, port))
        .unwrap_or_else(|_| panic!("{}", format!("Unable to connect to {}:{}", ip, port)));

    for _ in 0..repeats {
        // open workload file
        let workload = m3::vfs::VFS::open(wl, OpenFlags::R).expect("Could not open file");

        // Load workload info for the benchmark
        let mut workload_buffer = BufReader::new(workload);
        let workload_header = importer::WorkloadHeader::load_from_file(&mut workload_buffer);

        for _ in 0..workload_header.number_of_operations {
            let operation = importer::Package::load_as_bytes(&mut workload_buffer);
            debug_assert!(importer::Package::from_bytes(&operation).is_ok());

            if VERBOSE {
                println!("Sending operation...");
            }

            socket
                .send(&(operation.len() as u32).to_be_bytes())
                .expect("send failed");
            socket.send(&operation).expect("send failed");

            if VERBOSE {
                println!("Receiving response...");
            }

            let mut resp_bytes = [0u8; 8];
            socket
                .recv(&mut resp_bytes)
                .expect("receive response header failed");
            let resp_len = u64::from_be_bytes(resp_bytes);

            if VERBOSE {
                println!("Expecting {} byte response.", resp_len);
            }

            let mut response = vec![0u8; resp_len as usize];
            let mut rem = resp_len as usize;
            while rem > 0 {
                let amount = socket
                    .recv(&mut response[resp_len as usize - rem..])
                    .expect("receive response failed");
                rem -= amount;
            }

            if VERBOSE {
                println!("Got response.");
            }
        }

        let end_msg = b"ENDNOW";
        socket.send(&(end_msg.len() as u32).to_be_bytes()).unwrap();
        socket.send(end_msg).unwrap();
    }
}

#[no_mangle]
pub fn main() -> i32 {
    let args: Vec<_> = env::args().collect();
    if args.len() < 2 {
        usage();
    }

    let nm = NetworkManager::new("net").expect("Could not connect to network manager");

    if args[1] == "udp" {
        if args.len() != 3 {
            usage();
        }

        let port = args[2].parse::<Port>().expect("Failed to parse port");
        udp_receiver(&nm, port);
    }
    else {
        if args.len() != 6 {
            usage();
        }

        let ip = args[2]
            .parse::<IpAddr>()
            .expect("Failed to parse IP address");
        let port = args[3].parse::<Port>().expect("Failed to parse port");
        let repeats = args[5].parse::<u32>().expect("Failed to parse repeats");
        tcp_sender(&nm, ip, port, args[4], repeats);
    }

    0
}
