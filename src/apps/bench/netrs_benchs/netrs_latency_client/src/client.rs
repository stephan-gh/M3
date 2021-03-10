/*
 * Copyright (C) 2021, Tendsin Mende <tendsin.mende@mailbox.tu-dresden.de>
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
#[no_std]
#[macro_use]
extern crate m3;

use m3::{
    cell::StaticCell,
    col::{BoxList, BoxRef},
    com::Semaphore,
    net::{IpAddr, TcpSocket, UdpSocket},
    println, profile,
    profile::Profiler,
    session::NetworkManager,
    test::{self, WvTester},
    time::{self, Time},
};

// TODO that's hacky, but the only alternative I can see is to pass the WvTester to every single
// test case and every single wv_assert_* call, which is quite inconvenient.
static FAILED: StaticCell<u32> = StaticCell::new(0);

extern "C" fn wvtest_failed() {
    FAILED.set(*FAILED + 1);
}

struct MyTester {}

static PACKETS_TO_SEND: usize = 105;
static PACKETS_TO_RECEIVE: usize = 100;
static BURST_SIZE: usize = 2;

struct NetContext<'a> {
    socket: UdpSocket<'a>,
    req_buffer: [u8; 1024],
    dest_addr: IpAddr,
    dest_port: u16,
    sent_count: usize,
    receive_count: usize,
    received_bytes: usize,
}

impl WvTester for MyTester {
    fn run_suite(&mut self, name: &str, f: &dyn Fn(&mut dyn WvTester)) {
        println!("Running benchmark suite {} ...\n", name);
        f(self);
        println!();
    }

    fn run_test(&mut self, name: &str, file: &str, f: &dyn Fn()) {
        println!("Testing \"{}\" in {}:", name, file);
        f();
        println!();
    }
}

fn run(t: &mut dyn test::WvTester) {
    wv_run_test!(t, simple_bandwidth);
}

fn simple_bandwidth() {
    let mut prof = Profiler::default().repeats(5);


    let nm = NetworkManager::new("net0").unwrap();
    let mut socket = UdpSocket::new(&nm).unwrap();

    socket.set_blocking(true);
    Semaphore::attach("net").unwrap().down();

    socket.bind(IpAddr::new(192, 168, 112, 2), 1337).unwrap();


    let samples = 5;
    let dest_addr = IpAddr::new(192, 168, 112, 1);
    let dest_port = 1337;

    let mut warmup = 5;
    println!("Warmup...");
    let warump_bytes = [0; 1024];
    while warmup > 0 {
        warmup -= 1;
        socket.send(dest_addr, dest_port, &warump_bytes);
        let _pkg = socket.recv();
    }

    println!("warump done.\nBenchmark...");

    let packet_sizes = [8, 16, 32, 64, 128, 256, 512, 1024];
    let mut package = warump_bytes;

    wv_perf!(
        "running bandwidth test",
        prof.run_with_id(
            || {
                for pkt_size in &packet_sizes {
                    let mut res = vec![0; samples];
                    for i in 0..samples {
                        let start = time::start(i);
                        package[0..8].copy_from_slice(&start.to_be_bytes());

                        socket.send(dest_addr, dest_port, &package[0..*pkt_size]);
                        let send_len = pkt_size;

                        let pkg = socket.recv().expect("Got empty package");
                        let recv_len = pkg.size;
                        let stop = time::stop(i);

                        wv_assert!(*send_len == recv_len as usize);
                        let recved_time = u64::from_be_bytes([
                            pkg.raw_data()[0],
                            pkg.raw_data()[1],
                            pkg.raw_data()[2],
                            pkg.raw_data()[3],
                            pkg.raw_data()[4],
                            pkg.raw_data()[5],
                            pkg.raw_data()[6],
                            pkg.raw_data()[7],
                        ]);

                        wv_assert!((recv_len as usize) == *pkt_size || start == recved_time);

                        //println!("RTT ({}): {} cycles / {} ms (@3Ghz)", pkt_size, stop-start, (stop - start) / 0x3e6f);

                        res[i] = stop - start;
                    }
                    let avg = res.iter().sum::<u64>() / res.len() as u64;
                    //println!("network latency({}b) {}ms (+/- {} with {} runs)", pkt_size, avg / 0x3e6f, "unknown", res.len());
                }
            },
            0xa1
        )
    );
}

#[no_mangle]
pub fn main() -> i32 {
    let mut tester = MyTester {};
    wv_run_suite!(tester, run);

    println!("Finished");
    if *FAILED > 0 {
        println!("\x1B[1;31m{} tests failed\x1B[0;m", *FAILED);
    }
    else {
        println!("\x1B[1;32mAll tests successful!\x1B[0;m");
    }

    0
}
