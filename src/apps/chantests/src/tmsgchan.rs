/*
 * Copyright (C) 2019-2022 Nils Asmussen, Barkhausen Institut
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

use m3::chan::msgs as msgschan;
use m3::errors::Code;
use m3::test::WvTester;
use m3::tiles::{Activity, ActivityArgs, ChildActivity, RunningActivity, Tile};
use m3::{run_with_channels, wv_assert_eq, wv_assert_ok, wv_run_test};

pub fn run(t: &mut dyn WvTester) {
    wv_run_test!(t, test_chan);
    wv_run_test!(t, test_chan_macro);
}

fn test_chan(t: &mut dyn WvTester) {
    let (tx, rx) = wv_assert_ok!(msgschan::create());
    let (res_tx, res_rx) = wv_assert_ok!(msgschan::create());

    let tile = wv_assert_ok!(Tile::get("compat|own"));
    let mut act = wv_assert_ok!(ChildActivity::new_with(tile, ActivityArgs::new("test")));

    wv_assert_ok!(act.delegate_obj(rx.sel()));
    wv_assert_ok!(act.delegate_obj(res_tx.sel()));

    let mut sink = act.data_sink();
    sink.push(rx.sel());
    sink.push(res_tx.sel());

    let act = wv_assert_ok!(act.run(|| {
        let mut source = Activity::own().data_source();
        let rx0 = wv_assert_ok!(msgschan::Receiver::new_bind(source.pop()?));
        let res_tx0 = wv_assert_ok!(msgschan::Sender::new_bind(source.pop()?));

        let i1 = rx0.recv::<u32>()?;
        let res = (i1 + 5) as i32;
        res_tx0.send(res)?;
        Ok(())
    }));

    // since there is no buffering inside the channels,
    // all communication needs to be done before we wait
    // for the activities to finish.
    let tx = wv_assert_ok!(tx.activate());
    let res_rx = wv_assert_ok!(res_rx.activate());
    wv_assert_ok!(tx.send::<u32>(42));
    let res: i32 = wv_assert_ok!(res_rx.recv());
    wv_assert_eq!(t, res, 42 + 5);

    wv_assert_eq!(t, act.wait(), Ok(Code::Success));
}

fn test_chan_macro(t: &mut dyn WvTester) {
    let (tx, rx) = wv_assert_ok!(msgschan::create());
    let (res_tx, res_rx) = wv_assert_ok!(msgschan::create());

    let tile = wv_assert_ok!(Tile::get("compat|own"));
    let act = wv_assert_ok!(ChildActivity::new_with(tile, ActivityArgs::new("test")));

    let act = wv_assert_ok!(run_with_channels!(
        act,
        |rx0: msgschan::Receiver, res_tx0: msgschan::Sender| {
            let i1 = rx0.recv::<u32>()?;
            let res = (i1 + 5) as i32;
            res_tx0.send(res)?;
            Ok(())
        }(rx, res_tx)
    ));

    let tx = wv_assert_ok!(tx.activate());
    let res_rx = wv_assert_ok!(res_rx.activate());
    wv_assert_ok!(tx.send::<u32>(42));
    let res: i32 = wv_assert_ok!(res_rx.recv());
    wv_assert_eq!(t, res, 42 + 5);

    wv_assert_eq!(t, act.wait(), Ok(Code::Success));
}
