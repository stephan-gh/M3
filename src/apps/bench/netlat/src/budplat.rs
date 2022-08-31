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

use m3::format;
use m3::net::{DGramSocket, DgramSocketArgs, Endpoint, Socket, UdpSocket};
use m3::session::NetworkManager;
use m3::test::WvTester;
use m3::time::{CycleInstant, Results, TimeDuration};
use m3::vfs::{File, FileEvent, FileRef, FileWaiter};
use m3::{wv_assert_ok, wv_perf, wv_run_test};

const TIMEOUT: TimeDuration = TimeDuration::from_millis(30);

pub fn run(t: &mut dyn WvTester) {
    wv_run_test!(t, latency);
}

fn send_recv(waiter: &mut FileWaiter, socket: &mut FileRef<UdpSocket>, dest: Endpoint) -> bool {
    let mut buf = [0u8; 1];

    wv_assert_ok!(socket.send_to(&buf, dest));

    waiter.wait_for(TIMEOUT);

    if socket.has_data() {
        let _res = socket.recv(&mut buf);
        true
    }
    else {
        false
    }
}

fn latency(_t: &mut dyn WvTester) {
    let nm = wv_assert_ok!(NetworkManager::new("net"));
    let mut socket = wv_assert_ok!(UdpSocket::new(DgramSocketArgs::new(nm)));

    wv_assert_ok!(socket.set_blocking(false));

    let samples = 100;
    let dest = Endpoint::new(crate::DST_IP.get(), crate::DST_PORT.get());

    let mut waiter = FileWaiter::default();
    waiter.add(socket.fd(), FileEvent::INPUT);

    // warmup
    for _ in 0..5 {
        send_recv(&mut waiter, &mut socket, dest);
    }

    let mut res = Results::new(samples);

    while res.runs() < samples {
        let start = CycleInstant::now();

        if send_recv(&mut waiter, &mut socket, dest) {
            let stop = CycleInstant::now();
            res.push(stop.duration_since(start));
        }
    }

    wv_perf!(
        "UDP latency",
        format!(
            "{:?} (+/- {:?} with {} runs)",
            res.avg(),
            res.stddev(),
            res.runs()
        )
    );
}
