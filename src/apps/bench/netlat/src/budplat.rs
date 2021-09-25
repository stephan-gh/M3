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

use m3::format;
use m3::net::{DgramSocketArgs, Endpoint, UdpSocket};
use m3::profile::Results;
use m3::session::NetworkManager;
use m3::test;
use m3::time;
use m3::{wv_assert_ok, wv_perf, wv_run_test};

pub fn run(t: &mut dyn test::WvTester) {
    wv_run_test!(t, latency);
}

fn latency() {
    let nm = wv_assert_ok!(NetworkManager::new("net"));
    let mut socket = wv_assert_ok!(UdpSocket::new(DgramSocketArgs::new(&nm)));

    wv_assert_ok!(socket.bind(2000));

    let samples = 50;
    let dest = Endpoint::new(crate::DST_IP.get(), crate::DST_PORT.get());

    let mut buf = [0u8; 1];

    // warmup
    for _ in 0..5 {
        wv_assert_ok!(socket.send_to(&buf, dest));
        let _res = socket.recv(&mut buf);
    }

    let mut res = Results::new(samples);

    for i in 0..samples {
        let start = time::start(i);

        wv_assert_ok!(socket.send_to(&buf, dest));
        wv_assert_ok!(socket.recv(&mut buf));

        let stop = time::stop(i);

        res.push(stop - start);
    }

    wv_perf!(
        "UDP latency",
        format!(
            "{} cycles (+/- {} with {} runs)",
            res.avg(),
            res.stddev(),
            res.runs()
        )
    );
}
