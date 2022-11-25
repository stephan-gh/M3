/*
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

use m3::com::Semaphore;
use m3::errors::Code;
use m3::format;
use m3::net::{Endpoint, Socket, StreamSocket, StreamSocketArgs, TcpSocket};
use m3::println;
use m3::session::NetworkManager;
use m3::test::WvTester;
use m3::time::{Results, TimeDuration, TimeInstant};
use m3::vfs::{File, FileEvent, FileWaiter};
use m3::{wv_assert_eq, wv_assert_ok, wv_perf, wv_run_test};

pub fn run(t: &mut dyn WvTester) {
    wv_run_test!(t, latency);
    wv_run_test!(t, bandwidth);
}

fn latency(t: &mut dyn WvTester) {
    let nm = wv_assert_ok!(NetworkManager::new("net"));
    let mut socket = wv_assert_ok!(TcpSocket::new(StreamSocketArgs::new(nm)));

    wv_assert_ok!(Semaphore::attach("net-tcp").unwrap().down());

    wv_assert_ok!(socket.connect(Endpoint::new(crate::DST_IP.get(), 1338)));

    let samples = 5;

    let mut buf = [0u8; 1024];

    // warmup
    for _ in 0..5 {
        wv_assert_eq!(t, socket.send(&buf), Ok(buf.len()));
        let _res = socket.recv(&mut buf);
    }

    let packet_sizes = [8, 16, 32, 64, 128, 256, 512, 1024];

    for pkt_size in &packet_sizes {
        let mut res = Results::new(samples);

        for _ in 0..samples {
            let start = TimeInstant::now();

            wv_assert_eq!(t, socket.send(&buf[0..*pkt_size]), Ok(*pkt_size));

            let mut recv_size = 0;
            while recv_size < *pkt_size {
                recv_size += wv_assert_ok!(socket.recv(&mut buf));
            }

            let stop = TimeInstant::now();

            wv_assert_eq!(t, *pkt_size, recv_size);
            res.push(stop.duration_since(start));
        }

        wv_perf!(
            format!("network latency ({}b)", pkt_size),
            format!(
                "{:.4} ms (+/- {:.4} ms with {} runs)",
                res.avg().as_nanos() as f32 / 1e6,
                res.stddev().as_nanos() as f32 / 1e6,
                res.runs()
            )
        );
    }

    wv_assert_ok!(socket.close());
}

fn bandwidth(t: &mut dyn WvTester) {
    const PACKETS_TO_SEND: usize = 105;
    const BURST_SIZE: usize = 2;
    const TIMEOUT: TimeDuration = TimeDuration::from_secs(1);

    let nm = wv_assert_ok!(NetworkManager::new("net"));
    let mut socket = wv_assert_ok!(TcpSocket::new(
        StreamSocketArgs::new(nm)
            .send_buffer(64 * 1024)
            .recv_buffer(256 * 1024)
    ));

    wv_assert_ok!(Semaphore::attach("net-tcp").unwrap().down());

    wv_assert_ok!(socket.connect(Endpoint::new(crate::DST_IP.get(), 1338)));

    let mut buf = [0u8; 1024];

    for _ in 0..10 {
        wv_assert_eq!(t, socket.send(&buf), Ok(buf.len()));
        wv_assert_ok!(socket.recv(&mut buf));
    }

    wv_assert_ok!(socket.set_blocking(false));

    let start = TimeInstant::now();
    let mut timeout = start + TIMEOUT;
    let mut sent_count = 0;
    let mut receive_count = 0;
    let mut received_bytes = 0;
    let mut failures = 0;

    let mut waiter = FileWaiter::default();
    waiter.add(socket.fd(), FileEvent::INPUT | FileEvent::OUTPUT);

    loop {
        if failures > 9 {
            failures = 0;
            if sent_count >= PACKETS_TO_SEND {
                let rem = timeout.checked_duration_since(TimeInstant::now());
                match rem {
                    Some(d) => {
                        // we are not interested in output anymore
                        waiter.remove(socket.fd());
                        waiter.add(socket.fd(), FileEvent::INPUT);
                        waiter.wait_for(d);
                    },
                    None => break,
                }
            }
            else {
                waiter.wait();
            }
        }

        for _ in 0..BURST_SIZE {
            if sent_count >= PACKETS_TO_SEND {
                break;
            }

            match socket.send(&buf) {
                Err(e) => {
                    wv_assert_eq!(t, e.code(), Code::WouldBlock);
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
                    wv_assert_eq!(t, e.code(), Code::WouldBlock);
                    failures += 1;
                },
                Ok(size) => {
                    received_bytes += size;
                    receive_count += 1;
                    timeout = TimeInstant::now() + TIMEOUT;
                    failures = 0;
                },
            }
        }

        if received_bytes >= PACKETS_TO_SEND * buf.len() {
            break;
        }
    }

    println!("Sent packets: {}", sent_count);
    println!("Received packets: {}", receive_count);
    println!("Received bytes: {}", received_bytes);
    let last_received = timeout - TIMEOUT;
    let duration = last_received.duration_since(start);
    println!("Duration: {:?}", duration);
    let secs = (duration.as_nanos() as f64) / 1e9;
    let mbps = (received_bytes as f64 / secs) / (1024f64 * 1024f64);
    wv_perf!(
        "TCP bandwidth",
        format!("{} MiB/s (+/- 0 with 1 runs)", mbps)
    );

    wv_assert_ok!(socket.set_blocking(true));
    wv_assert_ok!(socket.close());
}
