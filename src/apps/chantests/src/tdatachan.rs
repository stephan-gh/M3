/*
 * Copyright (C) 2024 Nils Asmussen, Barkhausen Institut
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

use core::fmt::Debug;
use core::ops::{Add, AddAssign};

use m3::chan::data::{
    self as datachan, BlockReceiver, BlockSender, Receiver, ReceiverCap, ReceiverDesc, Sender,
    SenderCap, SenderDesc,
};
use m3::col::{String, ToString};
use m3::errors::{Code, Error};
use m3::io::LogFlags;
use m3::mem::{GlobOff, VirtAddr};
use m3::serialize::{Deserialize, Serialize};
use m3::test::WvTester;
use m3::tiles::{Activity, ChildActivity, RunningActivity, RunningProgramActivity};
use m3::time::{CycleDuration, Duration};
use m3::{log, wv_assert_ok};
use m3::{wv_assert_eq, wv_run_test};

use crate::create_data;
use crate::utils;

pub fn run(t: &mut dyn WvTester) {
    wv_run_test!(t, chain_u8);
    wv_run_test!(t, chain_u32);
}

fn chain_u8(t: &mut dyn WvTester) {
    let (input, output) = create_data!(1024, u8, 1 + 2);
    run_chain::<u8>(t, &input, &output, 1, 32, 64, 256);
    run_chain::<u8>(t, &input, &output, 2, 64, 32, 512);
}

fn chain_u32(t: &mut dyn WvTester) {
    let (input, output) = create_data!(256, u32, 1 + 2);
    run_chain::<u32>(t, &input, &output, 1, 32, 32, 256);
    run_chain::<u32>(t, &input, &output, 2, 64, 256, 512);
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(crate = "m3::serde")]
struct NodeConfig<T> {
    name: String,
    add: T,
    comp_time: CycleDuration,
    chunk_size: usize,
    recv: ReceiverDesc,
    send: Option<SenderDesc>,
}

fn start_activity<S: ToString, T: Serialize>(
    name: S,
    add: T,
    mut act: ChildActivity,
    recv: &ReceiverCap,
    send: Option<&SenderCap>,
    comp_time: CycleDuration,
    chunk_size: usize,
    func: fn() -> Result<(), Error>,
) -> Result<RunningProgramActivity, Error> {
    recv.delegate(&act)?;
    if let Some(send) = send {
        send.delegate(&act)?;
    }

    let mut dst = act.data_sink();
    dst.push(NodeConfig {
        name: name.to_string(),
        add,
        comp_time,
        chunk_size,
        recv: recv.desc(),
        send: send.map(|s| s.desc()),
    });

    act.run(func)
}

fn compute_node<'a: 'static, T>() -> Result<(), Error>
where
    T: Debug + Clone + Add<Output = T> + AddAssign + Serialize + Deserialize<'a>,
{
    let mut src = Activity::own().data_source();
    let cfg: NodeConfig<T> = wv_assert_ok!(src.pop());

    let recv = wv_assert_ok!(Receiver::new(cfg.name.clone(), cfg.recv));
    let mut send = cfg
        .send
        .map(|s| wv_assert_ok!(Sender::new(cfg.name.clone(), s)));

    log!(LogFlags::Debug, "{}: starting", cfg.name);

    for mut blk in recv.iter::<T, T>() {
        for chk in blk.buf_mut().chunks_mut(cfg.chunk_size) {
            utils::compute_for(&cfg.name, cfg.comp_time);

            for b in chk.iter_mut() {
                *b += cfg.add.clone();
            }
        }

        if let Some(send) = send.as_mut() {
            let user = blk.user().clone() + cfg.add.clone();
            wv_assert_ok!(send.send(blk, user));
        }
    }

    log!(LogFlags::Debug, "{}: finished", cfg.name);

    Ok(())
}

fn run_chain<'a: 'static, T>(
    t: &mut dyn WvTester,
    input: &[T],
    expected: &[T],
    credits: u32,
    chunk_size1: usize,
    chunk_size2: usize,
    buf_size: GlobOff,
) where
    T: Clone + Debug + Add<Output = T> + AddAssign + PartialEq + Eq + Serialize + Deserialize<'a>,
{
    const MSG_SIZE: usize = 128;
    const CHUNK_TIME: u64 = 100;
    const BUFFER_ADDR: VirtAddr = VirtAddr::new(0x3000_0000);

    let n1 = wv_assert_ok!(utils::create_activity("n1"));
    let n2 = wv_assert_ok!(utils::create_activity("n2"));

    let (n0n1_s, n0n1_r) = wv_assert_ok!(datachan::create(
        &n1,
        MSG_SIZE,
        credits,
        BUFFER_ADDR,
        buf_size
    ));
    let (n1n2_s, n1n2_r) = wv_assert_ok!(datachan::create(
        &n2,
        MSG_SIZE,
        credits,
        BUFFER_ADDR,
        buf_size
    ));
    let (n2n0_s, n2n0_r) = wv_assert_ok!(datachan::create(
        Activity::own(),
        MSG_SIZE,
        credits,
        BUFFER_ADDR,
        buf_size
    ));

    let n1 = wv_assert_ok!(start_activity(
        "n1",
        1,
        n1,
        &n0n1_r,
        Some(&n1n2_s),
        CycleDuration::from_raw(CHUNK_TIME * 2),
        chunk_size1,
        compute_node::<T>,
    ));

    let n2 = wv_assert_ok!(start_activity(
        "n2",
        2,
        n2,
        &n1n2_r,
        Some(&n2n0_s),
        CycleDuration::from_raw(CHUNK_TIME),
        chunk_size2,
        compute_node::<T>,
    ));

    let mut chan_n0n1 = wv_assert_ok!(Sender::new("n0", n0n1_s.desc()));
    let mut chan_n2n0 = wv_assert_ok!(Receiver::new("n0", n2n0_r.desc()));

    let user = input[42].clone();
    let mut pos = 0;
    wv_assert_ok!(datachan::pass_through(
        &mut chan_n0n1,
        &mut chan_n2n0,
        input,
        true,
        user,
        |blk| {
            wv_assert_eq!(t, blk.user().clone(), expected[42]);
            wv_assert_eq!(t, blk.buf(), &expected[pos..pos + blk.buf().len()]);
            pos += blk.buf().len();
        }
    ));

    wv_assert_eq!(t, n1.wait(), Ok(Code::Success));
    wv_assert_eq!(t, n2.wait(), Ok(Code::Success));
}
