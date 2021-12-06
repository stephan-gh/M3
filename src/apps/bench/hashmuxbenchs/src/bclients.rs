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
use m3::boxed::Box;
use m3::cap::Selector;
use m3::col::Vec;
use m3::com::{
    recv_msg, recv_reply, GateIStream, MemGate, Perm, RecvGate, SGateArgs, SendGate, EP,
};
use m3::crypto::HashAlgorithm;
use m3::errors::Error;
use m3::mem::MsgBuf;
use m3::pes::{Activity, ClosureActivity, PE, VPE};
use m3::profile::Results;
use m3::session::HashSession;
use m3::tcu::INVALID_EP;
use m3::{format, log, println, send_recv, time, wv_assert_ok, wv_run_test};
use m3::{math, mem, tcu, test};

pub fn run(t: &mut dyn test::WvTester) {
    wv_run_test!(t, hashmux_clients);
}

fn _create_rgate(max_clients: usize) -> RecvGate {
    let msg_size = math::next_log2(mem::size_of::<tcu::Header>() + mem::size_of::<u64>());
    let mut rgate = wv_assert_ok!(RecvGate::new(
        math::next_log2(max_clients) + msg_size,
        msg_size
    ));
    wv_assert_ok!(rgate.activate());
    rgate
}

struct Client {
    _sgate: SendGate,
    mgate: MemGate,
    act: ClosureActivity,
}

struct ClientParams {
    num: usize,
    algo: &'static HashAlgorithm,
    size: usize,
    div: usize,
    warm: u32,
    runs: u32,
}

/// Synchronize clients before each benchmark run or just the first one?
const SYNC_EVERY_RUN: bool = false;
const LOG_REQUESTS: bool = false;

fn _run_client_bench<F>(params: &ClientParams, sgate_sel: Selector, mut fun: F) -> Results
where
    F: FnMut(&HashSession) -> Result<(), Error>,
{
    let sgate = SendGate::new_bind(sgate_sel);

    // Use a separate RecvGate for replies since this is used
    // in parallel with other requests later
    let rgate = _create_rgate(1);

    let name = format!("hash-client{}", params.num);

    let mut res = Results::new(params.runs as usize);
    if SYNC_EVERY_RUN {
        for i in 0..(params.warm + params.runs) {
            let hash = wv_assert_ok!(HashSession::new(&name, params.algo));

            // Wait until everyone is ready to start
            wv_assert_ok!(send_recv!(&sgate, &rgate, hash.ep().sel()));

            let start = time::start(0x440);
            fun(&hash).unwrap();
            let end = time::stop(0x440);

            if i >= params.warm {
                res.push(end - start);
            }

            // Notify that the run is complete
            wv_assert_ok!(sgate.send(&MsgBuf::borrow_def(), &rgate));

            for num in 1.. {
                if fun(&hash).is_err() {
                    log!(num > 1, "Warning: Had to start {} extra runs", num);
                    break;
                }
            }

            // Wait for reply
            wv_assert_ok!(recv_reply(&rgate, Some(&sgate)));
        }
    }
    else {
        let hash = wv_assert_ok!(HashSession::new(&name, params.algo));

        // Wait until everyone is ready to start
        wv_assert_ok!(send_recv!(&sgate, &rgate, hash.ep().sel()));

        for i in 0..(params.warm + params.runs) {
            let start = time::start(0x441);
            fun(&hash).unwrap();
            let end = time::stop(0x441);

            if i >= params.warm {
                res.push(end - start);
            }
        }

        // Notify that the run is complete
        wv_assert_ok!(sgate.send(&MsgBuf::borrow_def(), &rgate));

        // Keep runnning for other clients that need more time
        for num in 1.. {
            if fun(&hash).is_err() {
                log!(num > 1, "Done after {} extra runs", num);
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
    let pe = wv_assert_ok!(PE::new(VPE::cur().pe_desc()));
    let mut vpe = wv_assert_ok!(VPE::new(pe, &format!("hash-c{}", params.num)));

    let sgate = wv_assert_ok!(SendGate::new_with(
        SGateArgs::new(rgate)
            .credits(1)
            .label(params.num as tcu::Label)
    ));
    let sgate_sel = sgate.sel();
    wv_assert_ok!(vpe.delegate_obj(sgate_sel));

    let mgate = wv_assert_ok!(mgate.derive(0, params.size, Perm::R));

    assert_eq!(params.size % params.div, 0);
    let slice = params.size / params.div;

    Client {
        _sgate: sgate,
        mgate,
        act: wv_assert_ok!(vpe.run(Box::new(move || {
            let res = _run_client_bench(&params, sgate_sel, |hash| {
                for off in (0..params.size).step_by(slice) {
                    log!(LOG_REQUESTS, "Sending request off {} len {}", off, slice);
                    hash.input(off, slice)?;
                }
                Ok(())
            });

            log!(
                true,
                "PERF \"hash {} bytes (slice: {} bytes) with {}\": {}\nthroughput {:.8} bytes/cycle",
                params.size,
                slice,
                params.algo.name,
                res,
                params.size as f32 / res.avg() as f32
            );
            0
        }))),
    }
}

fn _sync_clients<F, R>(rgate: &RecvGate, num: usize, action: F) -> R
where
    F: FnOnce(&mut [GateIStream]) -> R,
{
    // Collect messages from all clients
    let mut msgs: Vec<GateIStream> = Vec::with_capacity(num);
    while msgs.len() != num {
        msgs.push(wv_assert_ok!(recv_msg(&rgate)));
    }
    // Sort by client number
    msgs.sort_unstable_by_key(|msg| msg.label());

    let res = action(&mut msgs);

    // Reply to unblock clients again
    let empty_msg = MsgBuf::borrow_def();
    for mut msg in msgs {
        wv_assert_ok!(msg.reply(&empty_msg));
    }
    res
}

fn _sync_and_wait_for_clients(rgate: &RecvGate, mut clients: Vec<Client>) {
    loop {
        // Sync start of benchmark
        let mut eps: Vec<EP> = Vec::with_capacity(clients.len());
        let done = !_sync_clients(&rgate, clients.len(), |msgs| {
            for (i, msg) in msgs.iter_mut().enumerate() {
                let sel: Selector = wv_assert_ok!(msg.pop());
                if sel == 0 {
                    return false; // No EP sent, benchmark completed
                }

                // Obtain EP from VPE and configure it with the MemGate
                let sel = wv_assert_ok!(clients[i].act.vpe_mut().obtain_obj(sel));
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
        _sync_clients(&rgate, clients.len(), |_| {
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

fn hashmux_clients() {
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
                    algo: &HashAlgorithm::SHA3_256,
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
                    algo: &HashAlgorithm::SHA3_512,
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
