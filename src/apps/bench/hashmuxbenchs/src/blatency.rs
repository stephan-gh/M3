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

use m3::cap::Selector;
use m3::client::HashSession;
use m3::col::Vec;
use m3::com::{MemCap, MemGate, Perm, Semaphore};
use m3::crypto::HashAlgorithm;
use m3::io::LogFlags;
use m3::mem::GlobOff;
use m3::test::WvTester;
use m3::tiles::{Activity, ChildActivity, RunningActivity, RunningProgramActivity, Tile};
use m3::time::{CycleDuration, CycleInstant, Results, TimeDuration, TimeInstant};
use m3::{format, log, println, wv_assert_ok, wv_perf, wv_run_test};

const TEST_ALGO: &HashAlgorithm = &HashAlgorithm::SHA3_256;
const SLOW_ALGO: &HashAlgorithm = &HashAlgorithm::SHA3_512;

pub fn run(t: &mut dyn WvTester) {
    wv_run_test!(t, small_client_latency);
}

struct Client {
    _mcap: MemCap,
    act: RunningProgramActivity,
}

fn _start_background_client(num: usize, mgate: &MemGate, sem: &Semaphore, size: usize) -> Client {
    log!(LogFlags::Info, "Starting client {}", num);

    let tile = wv_assert_ok!(Tile::new(Activity::own().tile_desc()));
    let mut act = wv_assert_ok!(ChildActivity::new(tile, &format!("hash-c{}", num)));
    let mcap = wv_assert_ok!(mgate.derive_cap(0, size as GlobOff, Perm::R));

    wv_assert_ok!(act.delegate_obj(sem.sel()));
    wv_assert_ok!(act.delegate_obj(mcap.sel()));

    let mut dst = act.data_sink();
    dst.push(sem.sel());
    dst.push(mcap.sel());
    dst.push(num);
    dst.push(size);

    Client {
        _mcap: mcap,
        act: wv_assert_ok!(act.run(|| {
            let mut src = Activity::own().data_source();
            let sem_sel: Selector = src.pop().unwrap();
            let mgate_sel: Selector = src.pop().unwrap();
            let num: usize = src.pop().unwrap();
            let size: usize = src.pop().unwrap();

            let sem = Semaphore::bind(sem_sel);
            let hash = wv_assert_ok!(HashSession::new(&format!("hash-client{}", num), SLOW_ALGO));
            wv_assert_ok!(hash.ep().configure(mgate_sel));

            // Notify main Tile that client is ready
            wv_assert_ok!(sem.up());

            loop {
                log!(LogFlags::Info, "Starting to hash {} bytes", size);
                wv_assert_ok!(hash.input(0, size));
            }
        })),
    }
}

const WARM: usize = 4;
const RUNS: usize = 10;
const TOTAL: usize = WARM + RUNS;

// Must be power of two for simplicity
const WAIT_MASK: u64 = 262144 - 1;

fn _bench_latency(mgate: &MemGate, size: usize) -> Results<CycleDuration> {
    let hash = wv_assert_ok!(HashSession::new("hash-latency", TEST_ALGO));
    let mgated = wv_assert_ok!(mgate.derive_cap(0, size as GlobOff, Perm::R));
    wv_assert_ok!(hash.ep().configure(mgated.sel()));

    // Read pseudo random memory from the memory region filled with SHAKE earlier
    let mut waits: [u64; TOTAL] = wv_assert_ok!(mgate.read_obj(0));
    for wait in &mut waits {
        // Mask out higher bits to have somewhat reasonable wait times
        *wait &= WAIT_MASK;
    }

    let mut res = Results::new(RUNS);
    for (i, &wait) in waits.iter().enumerate() {
        {
            // Spin for the chosen amount of time
            let end = TimeInstant::now() + TimeDuration::from_nanos(wait);
            log!(LogFlags::Info, "Waiting {} ns", wait);
            while TimeInstant::now() < end {}
        }

        let start = CycleInstant::now();
        hash.input(0, size).unwrap();
        let end = CycleInstant::now();

        if i >= WARM {
            res.push(end.duration_since(start));
        }
    }
    res
}

fn small_client_latency(_t: &mut dyn WvTester) {
    const LARGE_SIZE: usize = 16 * 1024 * 1024; // 16 MiB
    const SMALL_SIZE: usize = 512;

    log!(
        LogFlags::Info,
        "Preparing {} bytes using SHAKE...",
        LARGE_SIZE
    );
    let mgate = util::prepare_shake_mem(LARGE_SIZE);

    // Start clients that produce background load
    for order in 0..=1 {
        let count = 1 << order;
        let mut clients: Vec<Client> = Vec::with_capacity(count);

        println!("\nTesting latency with {} background clients", count);

        let sem = wv_assert_ok!(Semaphore::create(0));
        for c in 0..count {
            clients.push(_start_background_client(c, &mgate, &sem, LARGE_SIZE));
        }

        log!(LogFlags::Info, "Waiting for clients");
        for _ in 0..count {
            wv_assert_ok!(sem.down());
        }

        let res = _bench_latency(&mgate, SMALL_SIZE);

        log!(LogFlags::Info, "Stopping clients");
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
