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
use core::ops::{Add, AddAssign, Deref};

use m3::chan::data::{self as datachan, BlockReceiver, BlockSender};
use m3::chan::multidata::{
    self as mdatachan, MultiReceiver, MultiReceiverCap, MultiReceiverDesc, MultiSender,
    MultiSenderCap, MultiSenderDesc,
};
use m3::col::{String, ToString};
use m3::errors::{Code, Error};
use m3::io::LogFlags;
use m3::mem::GlobOff;
use m3::serialize::{Deserialize, Serialize};
use m3::test::WvTester;
use m3::tiles::{Activity, ChildActivity, RunningActivity, RunningProgramActivity};
use m3::time::{CycleDuration, Duration};
use m3::{log, wv_assert_ok};
use m3::{wv_assert_eq, wv_run_test};

use crate::create_data;
use crate::utils;

pub fn run(t: &mut dyn WvTester) {
    wv_run_test!(t, chain_in_place);
    wv_run_test!(t, chain_copy);
}

fn chain_in_place(t: &mut dyn WvTester) {
    let (ins, outs) = create_data!(1024, u8, 1 + 2);
    run_chain::<u8>(t, compute_in_place::<u8>, &ins, &outs, 1, 32, 32, 32, 256);
    run_chain::<u8>(t, compute_in_place::<u8>, &ins, &outs, 2, 64, 32, 64, 512);
}

fn chain_copy(t: &mut dyn WvTester) {
    let (ins, outs) = create_data!(512, u16, 1 + 2);
    run_chain::<u16>(t, compute_copy::<u16>, &ins, &outs, 1, 32, 32, 32, 256);
    run_chain::<u16>(t, compute_copy::<u16>, &ins, &outs, 2, 32, 64, 64, 1024);
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(crate = "m3::serde")]
struct NodeConfig<T> {
    add: T,
    name: String,
    comp_time: CycleDuration,
    chunk_size: usize,
    recv: MultiReceiverDesc,
    send: Option<MultiSenderDesc>,
}

fn start_activity<S: ToString, T: Serialize>(
    add: T,
    name: S,
    mut act: ChildActivity,
    recv: (&MultiReceiverCap, Option<usize>),
    send: Option<(&MultiSenderCap, Option<usize>)>,
    comp_time: CycleDuration,
    chunk_size: usize,
    func: fn() -> Result<(), Error>,
) -> Result<RunningProgramActivity, Error> {
    recv.0.delegate(&act)?;
    if let Some(send) = send {
        send.0.delegate(&act)?;
    }

    let mut dst = act.data_sink();
    dst.push(NodeConfig {
        add,
        name: name.to_string(),
        comp_time,
        chunk_size,
        recv: match recv {
            (cap, Some(idx)) => cap.desc_single(idx),
            (cap, None) => cap.desc(),
        },
        send: send.map(|s| match s {
            (cap, Some(idx)) => cap.desc_single(idx),
            (cap, None) => cap.desc(),
        }),
    });

    act.run(func)
}

fn compute_copy<'a: 'static, T>() -> Result<(), Error>
where
    T: Copy + Debug + Add<Output = T> + AddAssign + Serialize + Deserialize<'a>,
{
    let mut src = Activity::own().data_source();
    let cfg: NodeConfig<T> = wv_assert_ok!(src.pop());

    let recv = wv_assert_ok!(MultiReceiver::new(cfg.name.clone(), cfg.recv));
    let mut send = cfg
        .send
        .map(|s| wv_assert_ok!(MultiSender::new(cfg.name.clone(), s)));

    log!(LogFlags::Debug, "{}: starting", cfg.name);

    for mut mblk in recv.iter::<T, T>() {
        let last = mblk.is_last();
        let user = mblk.blocks()[0].user().clone();

        mblk.with_data(|data| {
            for chk in data.chunks_mut(cfg.chunk_size) {
                utils::compute_for(&cfg.name, cfg.comp_time);

                for b in chk.iter_mut() {
                    *b += cfg.add.clone();
                }
            }

            if let Some(send) = send.as_mut() {
                wv_assert_ok!(send.send_slice(&data, last, user + cfg.add));
            }
        })
    }

    log!(LogFlags::Debug, "{}: finished", cfg.name);
    Ok(())
}

fn compute_in_place<'a: 'static, T>() -> Result<(), Error>
where
    T: Clone + Debug + Add<Output = T> + AddAssign + Serialize + Deserialize<'a>,
{
    let mut src = Activity::own().data_source();
    let cfg: NodeConfig<T> = wv_assert_ok!(src.pop());

    let recv = wv_assert_ok!(MultiReceiver::new(cfg.name.clone(), cfg.recv));
    let mut send = cfg
        .send
        .map(|s| wv_assert_ok!(MultiSender::new(cfg.name.clone(), s)));

    log!(LogFlags::Debug, "{}: starting", cfg.name);

    for mut mblk in recv.iter::<T, T>() {
        let user = mblk.blocks()[0].user().clone();

        for blk in mblk.blocks_mut() {
            let comp_time = (blk.buf().len() / cfg.chunk_size) as u64 * cfg.comp_time.as_raw();
            utils::compute_for(&cfg.name, CycleDuration::new(comp_time));

            for b in blk.buf_mut().iter_mut() {
                *b += cfg.add.clone();
            }
        }

        if let Some(send) = send.as_mut() {
            wv_assert_ok!(send.send(mblk, user + cfg.add.clone()));
        }
    }

    log!(LogFlags::Debug, "{}: finished", cfg.name);
    Ok(())
}

fn run_chain<'a: 'static, T>(
    t: &mut dyn WvTester,
    func: fn() -> Result<(), Error>,
    input: &[T],
    expected: &[T],
    credits: u32,
    chunk_size1: usize,
    chunk_size2: usize,
    chunk_size3: usize,
    buf_size: GlobOff,
) where
    T: Clone + Debug + AddAssign + PartialEq + Eq + Serialize + Deserialize<'a>,
{
    const MSG_SIZE: usize = 128;
    const CHUNK_TIME: u64 = 100;

    let buf_addr = utils::buffer_addr();

    let n1 = wv_assert_ok!(utils::create_activity("n1"));
    let n2 = wv_assert_ok!(utils::create_activity("n2"));
    let n3 = wv_assert_ok!(utils::create_activity("n3"));

    let (n0n1_s, n0n1_r) = wv_assert_ok!(mdatachan::create_single(
        &n1, MSG_SIZE, credits, buf_addr, buf_size
    ));
    let (n1m_s, n1m_r) = wv_assert_ok!(mdatachan::create_fanout(
        [&n2, &n3].iter().map(|&a| a.deref()),
        MSG_SIZE,
        credits,
        buf_addr,
        buf_size
    ));
    let (mn0_s, mn0_r) = wv_assert_ok!(mdatachan::create_fanin(
        Activity::own(),
        MSG_SIZE,
        credits,
        buf_addr,
        buf_size,
        2
    ));

    let n1 = wv_assert_ok!(start_activity(
        1,
        "n1",
        n1,
        (&n0n1_r, None),
        Some((&n1m_s, None)),
        CycleDuration::from_raw(CHUNK_TIME),
        chunk_size1,
        func,
    ));

    let n2 = wv_assert_ok!(start_activity(
        2,
        "n2",
        n2,
        (&n1m_r, Some(0)),
        Some((&mn0_s, Some(0))),
        CycleDuration::from_raw(CHUNK_TIME),
        chunk_size2,
        func,
    ));

    let n3 = wv_assert_ok!(start_activity(
        2,
        "n3",
        n3,
        (&n1m_r, Some(1)),
        Some((&mn0_s, Some(1))),
        CycleDuration::from_raw(CHUNK_TIME),
        chunk_size3,
        func,
    ));

    let mut chan_n0n1 = wv_assert_ok!(MultiSender::new("n0", n0n1_s.desc()));
    let mut chan_mn0 = wv_assert_ok!(MultiReceiver::new("n0", mn0_r.desc()));

    let user = input[42].clone();
    let mut pos = 0;
    wv_assert_ok!(datachan::pass_through(
        &mut chan_n0n1,
        &mut chan_mn0,
        &input,
        true,
        user,
        |mblk| {
            wv_assert_eq!(t, mblk.blocks()[0].user().clone(), expected[42]);
            for blk in mblk.blocks() {
                wv_assert_eq!(t, blk.buf(), &expected[pos..pos + blk.buf().len()]);
                pos += blk.buf().len();
            }
        }
    ));

    wv_assert_eq!(t, n1.wait(), Ok(Code::Success));
    wv_assert_eq!(t, n2.wait(), Ok(Code::Success));
    wv_assert_eq!(t, n3.wait(), Ok(Code::Success));
}
