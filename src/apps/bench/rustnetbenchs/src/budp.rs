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

use m3::com::Semaphore;
use m3::errors::Code;
use m3::format;
use m3::net::{DgramSocketArgs, IpAddr, UdpSocket};
use m3::pes::VPE;
use m3::println;
use m3::profile::Results;
use m3::session::NetworkManager;
use m3::test;
use m3::time;
use m3::{wv_assert_eq, wv_assert_ok, wv_perf, wv_run_test};

pub fn run(t: &mut dyn test::WvTester) {
    // wait once for UDP, because it's connection-less
    wv_assert_ok!(Semaphore::attach("net-udp").unwrap().down());

    wv_run_test!(t, latency);
    wv_run_test!(t, bandwidth);
}

fn latency() {
    let nm = wv_assert_ok!(NetworkManager::new("net0"));
    let mut socket = wv_assert_ok!(UdpSocket::new(DgramSocketArgs::new(&nm)));

    wv_assert_ok!(socket.bind(2000));

    let samples = 5;
    let dest_addr = IpAddr::new(192, 168, 112, 1);
    let dest_port = 1337;

    let mut buf = [0u8; 1024];

    // warmup
    for _ in 0..5 {
        wv_assert_ok!(socket.send_to(&buf, dest_addr, dest_port));
        let _res = socket.recv(&mut buf);
    }

    let packet_sizes = [8, 16, 32, 64, 128, 256, 512, 1024];

    for pkt_size in &packet_sizes {
        let mut res = Results::new(samples);

        for i in 0..samples {
            let start = time::start(i);

            wv_assert_ok!(socket.send_to(&buf[0..*pkt_size], dest_addr, dest_port));
            let recv_size = wv_assert_ok!(socket.recv(&mut buf));

            let stop = time::stop(i);

            wv_assert_eq!(*pkt_size, recv_size as usize);
            res.push(stop - start);
        }

        wv_perf!(
            format!("network latency ({}b)", pkt_size),
            format!(
                "{:.4} ms (+/- {} with {} runs)",
                res.avg() as f32 / 3000000.,
                res.stddev() / 3000000.,
                res.runs()
            )
        );
    }
}

fn bandwidth() {
    const PACKETS_TO_SEND: usize = 105;
    const PACKETS_TO_RECEIVE: usize = 100;
    const BURST_SIZE: usize = 2;
    const TIMEOUT: u64 = 10_000_000; // cycles

    let nm = wv_assert_ok!(NetworkManager::new("net0"));
    let mut socket = wv_assert_ok!(UdpSocket::new(
        DgramSocketArgs::new(&nm)
            .send_buffer(8, 64 * 1024)
            .recv_buffer(32, 256 * 1024)
    ));

    wv_assert_ok!(socket.bind(2001));

    let dest_addr = IpAddr::new(192, 168, 112, 1);
    let dest_port = 1337;

    let mut buf = [0u8; 1024];

    for _ in 0..10 {
        wv_assert_ok!(socket.send_to(&buf, dest_addr, dest_port));
        wv_assert_ok!(socket.recv(&mut buf));
    }

    socket.set_blocking(false);

    let start = time::start(0);
    let mut last_received = start;
    let mut sent_count = 0;
    let mut receive_count = 0;
    let mut received_bytes = 0;
    let mut failures = 0;

    loop {
        if failures > 9 {
            failures = 0;
            wv_assert_ok!(VPE::sleep());
        }

        for _ in 0..BURST_SIZE {
            if sent_count > PACKETS_TO_SEND {
                break;
            }

            match socket.send_to(&buf, dest_addr, dest_port) {
                Err(e) => {
                    wv_assert_eq!(e.code(), Code::WouldBlock);
                    failures += 1;
                },
                Ok(_) => {
                    sent_count += 1;
                    failures = 0;
                },
            }
        }

        for _ in 0..BURST_SIZE {
            match socket.recv(&mut buf) {
                Err(e) => {
                    wv_assert_eq!(e.code(), Code::WouldBlock);
                    failures += 1;
                },
                Ok(size) => {
                    received_bytes += size as usize;
                    receive_count += 1;
                    last_received = time::start(1);
                    failures = 0;
                },
            }
        }

        if receive_count >= PACKETS_TO_RECEIVE {
            break;
        }
        if sent_count >= PACKETS_TO_SEND && time::start(1) - last_received > TIMEOUT {
            break;
        }
    }

    println!("Sent packets: {}", sent_count);
    println!("Received packets: {}", receive_count);
    println!("Received bytes: {}", received_bytes);
    let duration = last_received - start;
    println!("Duration: {}", duration);
    let mbps = (received_bytes as f64 / (duration as f64 / 3e9)) / (1024f64 * 1024f64);
    wv_perf!(
        "UDP bandwidth",
        format!("{} MiB/s (+/- 0 with 1 runs)", mbps)
    );
}
