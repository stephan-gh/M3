/*
 * Copyright (C) 2021, Stephan Gerhold <stephan.gerhold@mailbox.tu-dresden.de>
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

use crate::util;
use m3::boxed::Box;
use m3::col::Vec;
use m3::com::{MemGate, Perm, Semaphore};
use m3::crypto::HashAlgorithm;
use m3::pes::{Activity, ClosureActivity, PE, VPE};
use m3::profile::Results;
use m3::session::HashSession;
use m3::tcu::TCU;
use m3::test;
use m3::{format, log, println, time, wv_assert_ok, wv_perf, wv_run_test};

const TEST_ALGO: &HashAlgorithm = &HashAlgorithm::SHA3_256;
const SLOW_ALGO: &HashAlgorithm = &HashAlgorithm::SHA3_512;
const LOG_DEBUG: bool = true;

pub fn run(t: &mut dyn test::WvTester) {
    wv_run_test!(t, small_client_latency);
}

struct Client {
    _mgate: MemGate,
    act: ClosureActivity,
}

fn _start_background_client(num: usize, mgate: &MemGate, sem: &Semaphore, size: usize) -> Client {
    log!(LOG_DEBUG, "Starting client {}", num);

    let pe = wv_assert_ok!(PE::new(VPE::cur().pe_desc()));
    let mut vpe = wv_assert_ok!(VPE::new(pe, &format!("hash-c{}", num)));
    let mgate = wv_assert_ok!(mgate.derive(0, size, Perm::R));

    let sem_sel = sem.sel();
    let mgate_sel = mgate.sel();
    wv_assert_ok!(vpe.delegate_obj(sem_sel));
    wv_assert_ok!(vpe.delegate_obj(mgate_sel));

    Client {
        _mgate: mgate,
        act: wv_assert_ok!(vpe.run(Box::new(move || {
            let sem = Semaphore::bind(sem_sel);
            let hash = wv_assert_ok!(HashSession::new(&format!("hash-client{}", num), SLOW_ALGO));
            wv_assert_ok!(hash.ep().configure(mgate_sel));

            // Notify main PE that client is ready
            wv_assert_ok!(sem.up());

            loop {
                log!(LOG_DEBUG, "Starting to hash {} bytes", size);
                wv_assert_ok!(hash.input(0, size));
            }
        }))),
    }
}

const WARM: usize = 10;
const RUNS: usize = 100;
const TOTAL: usize = WARM + RUNS;

// Must be power of two for simplicity
const WAIT_MASK: u32 = 262144 - 1;

fn _bench_latency(mgate: &MemGate, size: usize) -> Results {
    let hash = wv_assert_ok!(HashSession::new("hash-latency", TEST_ALGO));
    let mgated = wv_assert_ok!(mgate.derive(0, size, Perm::R));
    wv_assert_ok!(hash.ep().configure(mgated.sel()));

    // Read pseudo random memory from the memory region filled with SHAKE earlier
    let mut waits: [u32; TOTAL] = wv_assert_ok!(mgate.read_obj(0));
    for wait in &mut waits {
        // Mask out higher bits to have somewhat reasonable wait times
        *wait &= WAIT_MASK;
    }

    let mut res = Results::new(RUNS);
    for (i, &wait) in waits.iter().enumerate() {
        {
            // Spin for the chosen amount of time
            let end = TCU::nanotime() + wait as u64;
            log!(LOG_DEBUG, "Waiting {} ns", wait);
            while TCU::nanotime() < end {}
        }

        let start = time::start(0x444);
        hash.input(0, size).unwrap();
        let end = time::stop(0x444);

        if i >= WARM {
            res.push(end - start);
        }
    }
    res
}

fn small_client_latency() {
    const LARGE_SIZE: usize = 64 * 1024 * 1024; // 64 MiB
    const SMALL_SIZE: usize = 512;

    log!(LOG_DEBUG, "Preparing {} bytes using SHAKE...", LARGE_SIZE);
    let mgate = util::prepare_shake_mem(LARGE_SIZE);

    // Start clients that produce background load
    for order in 0..=3 {
        let count = 1 << order;
        let mut clients: Vec<Client> = Vec::with_capacity(count);

        println!("\nTesting latency with {} background clients", count);

        let sem = wv_assert_ok!(Semaphore::create(0));
        for c in 0..count {
            clients.push(_start_background_client(c, &mgate, &sem, LARGE_SIZE));
        }

        log!(LOG_DEBUG, "Waiting for clients");
        for _ in 0..count {
            wv_assert_ok!(sem.down());
        }

        let res = _bench_latency(&mgate, SMALL_SIZE);

        log!(LOG_DEBUG, "Stopping clients");
        for client in clients {
            wv_assert_ok!(client.act.stop());
        }

        wv_perf!(
            format!(
                "random wait hash {} bytes with {} ({} clients)",
                SMALL_SIZE, TEST_ALGO.name, count
            ),
            res
        );
    }
}
