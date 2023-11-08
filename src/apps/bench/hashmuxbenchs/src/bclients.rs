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
 * General Pubrlic License version 2 for more details.
 */

use crate::util;

use m3::cap::Selector;
use m3::client::HashSession;
use m3::col::Vec;
use m3::com::{
    recv_msg, recv_reply, GateIStream, MemGate, Perm, RecvGate, SGateArgs, SendCap, SendGate, EP,
};
use m3::crypto::{HashAlgorithm, HashType};
use m3::errors::Error;
use m3::io::LogFlags;
use m3::mem;
use m3::serialize::{Deserialize, Serialize};
use m3::tcu;
use m3::tcu::INVALID_EP;
use m3::test::WvTester;
use m3::tiles::{Activity, ChildActivity, RunningActivity, RunningProgramActivity, Tile};
use m3::time::{CycleDuration, CycleInstant, Duration, Results};
use m3::util::math;
use m3::{format, log, println, send_recv, wv_assert_ok, wv_run_test};

pub fn run(t: &mut dyn WvTester) {
    wv_run_test!(t, hashmux_clients);
}

fn _create_rgate(max_clients: usize) -> RecvGate {
    let msg_size = math::next_log2(mem::size_of::<tcu::Header>() + mem::size_of::<u64>());
    wv_assert_ok!(RecvGate::new(
        math::next_log2(max_clients) + msg_size,
        msg_size
    ))
}

struct Client {
    _scap: SendCap,
    mgate: MemGate,
    act: RunningProgramActivity,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "m3::serde")]
struct ClientParams {
    num: usize,
    algo: HashType,
    size: usize,
    div: usize,
    warm: u32,
    runs: u32,
}

/// Synchronize clients before each benchmark run or just the first one?
const SYNC_EVERY_RUN: bool = false;

fn _run_client_bench<F>(
    params: &ClientParams,
    sgate_sel: Selector,
    mut fun: F,
) -> Results<CycleDuration>
where
    F: FnMut(&HashSession) -> Result<(), Error>,
{
    let sgate = wv_assert_ok!(SendGate::new_bind(sgate_sel));

    // Use a separate RecvGate for replies since this is used
    // in parallel with other requests later
    let rgate = _create_rgate(1);

    let name = format!("hash-client{}", params.num);
    let algo = HashAlgorithm::from_type(params.algo).unwrap();

    let mut res = Results::new(params.runs as usize);
    if SYNC_EVERY_RUN {
        for i in 0..(params.warm + params.runs) {
            let hash = wv_assert_ok!(HashSession::new(&name, algo));

            // Wait until everyone is ready to start
            wv_assert_ok!(send_recv!(&sgate, &rgate, hash.ep().sel()));

            let start = CycleInstant::now();
            fun(&hash).unwrap();
            let end = CycleInstant::now();

            if i >= params.warm {
                res.push(end.duration_since(start));
            }

            // Notify that the run is complete
            wv_assert_ok!(sgate.send(&mem::MsgBuf::borrow_def(), &rgate));

            for num in 1.. {
                if fun(&hash).is_err() {
                    if num > 1 {
                        log!(LogFlags::Info, "Warning: Had to start {} extra runs", num);
                    }
                    break;
                }
            }

            // Wait for reply
            wv_assert_ok!(recv_reply(&rgate, Some(&sgate)));
        }
    }
    else {
        let hash = wv_assert_ok!(HashSession::new(&name, algo));

        // Wait until everyone is ready to start
        wv_assert_ok!(send_recv!(&sgate, &rgate, hash.ep().sel()));

        for i in 0..(params.warm + params.runs) {
            let start = CycleInstant::now();
            fun(&hash).unwrap();
            let end = CycleInstant::now();

            if i >= params.warm {
                res.push(end.duration_since(start));
            }
        }

        // Notify that the run is complete
        wv_assert_ok!(sgate.send(&mem::MsgBuf::borrow_def(), &rgate));

        // Keep runnning for other clients that need more time
        for num in 1.. {
            if fun(&hash).is_err() {
                if num > 1 {
                    log!(LogFlags::Info, "Done after {} extra runs", num);
                }
                break;
            }
        }

        // Wait for reply
        wv_assert_ok!(recv_reply(&rgate, Some(&sgate)));
    }

    wv_assert_ok!(send_recv!(&sgate, &rgate, 0));
    res
}

fn _start_client(params: ClientParams, rgate: &RecvGate, mgate: &MemGate) -> Client {
    let tile = wv_assert_ok!(Tile::new(Activity::own().tile_desc()));
    let mut act = wv_assert_ok!(ChildActivity::new(tile, &format!("hash-c{}", params.num)));

    let scap = wv_assert_ok!(SendCap::new_with(
        SGateArgs::new(rgate)
            .credits(1)
            .label(params.num as tcu::Label)
    ));
    wv_assert_ok!(act.delegate_obj(scap.sel()));

    let mgate = wv_assert_ok!(mgate.derive(0, params.size, Perm::R));

    assert_eq!(params.size % params.div, 0);
    let slice = params.size / params.div;

    let mut dst = act.data_sink();
    dst.push(scap.sel());
    dst.push(slice);
    dst.push(params);

    Client {
        _scap: scap,
        mgate,
        act: wv_assert_ok!(act.run(|| {
            let mut src = Activity::own().data_source();
            let sgate_sel: Selector = src.pop().unwrap();
            let slice: usize = src.pop().unwrap();
            let params: ClientParams = src.pop().unwrap();

            let res = _run_client_bench(&params, sgate_sel, |hash| {
                for off in (0..params.size).step_by(slice) {
                    log!(LogFlags::Debug, "Sending request off {} len {}", off, slice);
                    hash.input(off, slice)?;
                }
                Ok(())
            });

            log!(
                LogFlags::Info,
                "PERF \"hash {} bytes (slice: {} bytes) with {:?}\": {}\nthroughput {:.8} bytes/cycle",
                params.size,
                slice,
                params.algo,
                res,
                params.size as f32 / res.avg().as_raw() as f32
            );
            Ok(())
        })),
    }
}

fn _sync_clients<F, R>(rgate: &RecvGate, num: usize, action: F) -> R
where
    F: FnOnce(&mut [GateIStream<'_>]) -> R,
{
    // Collect messages from all clients
    let mut msgs: Vec<GateIStream<'_>> = Vec::with_capacity(num);
    while msgs.len() != num {
        msgs.push(wv_assert_ok!(recv_msg(rgate)));
    }
    // Sort by client number
    msgs.sort_unstable_by_key(|msg| msg.label());

    let res = action(&mut msgs);

    // Reply to unblock clients again
    let empty_msg = mem::MsgBuf::borrow_def();
    for mut msg in msgs {
        wv_assert_ok!(msg.reply(&empty_msg));
    }
    res
}

fn _sync_and_wait_for_clients(rgate: &RecvGate, mut clients: Vec<Client>) {
    loop {
        // Sync start of benchmark
        let mut eps: Vec<EP> = Vec::with_capacity(clients.len());
        let done = !_sync_clients(rgate, clients.len(), |msgs| {
            for (i, msg) in msgs.iter_mut().enumerate() {
                let sel: Selector = wv_assert_ok!(msg.pop());
                if sel == 0 {
                    return false; // No EP sent, benchmark completed
                }

                // Obtain EP from activity and configure it with the MemGate
                let sel = wv_assert_ok!(clients[i].act.activity_mut().obtain_obj(sel));
                let ep = EP::new_bind(INVALID_EP, sel);
                wv_assert_ok!(ep.configure(clients[i].mgate.sel()));
                eps.push(ep);
            }
            true
        });
        if done {
            break;
        }

        // Sync end of benchmark
        _sync_clients(rgate, clients.len(), |_| {
            for ep in eps {
                // Invalidate EP so additional runs cancel early
                // This will cause [0] hash::work() failed with NoMEP but this is expected
                wv_assert_ok!(ep.invalidate());
            }
        });
    }

    // Wait until everyone is done
    for client in clients {
        wv_assert_ok!(client.act.wait());
    }
}

fn hashmux_clients(_t: &mut dyn WvTester) {
    const MAX_CLIENTS: usize = 2;
    const MAX_SIZE: usize = 512 * 1024; // 512 KiB

    let mgate = util::prepare_shake_mem(MAX_SIZE);

    // For synchronization all clients sent a message and the reply
    // is only sent once the message from all clients has arrived.
    let rgate = _create_rgate(MAX_CLIENTS);

    // 2 that hash MAX_SIZE
    for order in 1..=1 {
        let count = 1 << order;
        let mut clients: Vec<Client> = Vec::with_capacity(count);

        println!("\nTesting {} clients with same buffer sizes...", count);

        for c in 0..count {
            clients.push(_start_client(
                ClientParams {
                    num: c,
                    algo: HashType::SHA3_256,
                    size: MAX_SIZE,
                    div: 1,
                    warm: 2,
                    runs: 5,
                },
                &rgate,
                &mgate,
            ));
        }

        _sync_and_wait_for_clients(&rgate, clients);
    }

    // 2 that hash MAX_SIZE fully or in slices
    for order in 1..=1 {
        let count = 1 << order;
        let mut clients: Vec<Client> = Vec::with_capacity(count);

        println!("\nTesting {} clients with different buffer sizes...", count);

        for c in 0..count {
            clients.push(_start_client(
                ClientParams {
                    num: c,
                    algo: HashType::SHA3_512,
                    size: MAX_SIZE,
                    div: if count <= 2 {
                        // Two client hashing with different slice size
                        1 << c
                    }
                    else {
                        // Always two clients with decreasing slice size
                        1 << (c / 2)
                    },
                    warm: 3,
                    runs: 5,
                },
                &rgate,
                &mgate,
            ));
        }

        _sync_and_wait_for_clients(&rgate, clients);
    }
}
