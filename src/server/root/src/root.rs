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

#![no_std]

#![feature(const_fn)]
#![feature(const_vec_new)]
#![feature(core_intrinsics)]

#[macro_use]
extern crate m3;

mod childs;
mod loader;
mod services;

use core::intrinsics;
use m3::boxed::Box;
use m3::cap::Selector;
use m3::col::{String, ToString, Vec};
use m3::com::{GateIStream, MemGate, RecvGate, RGateArgs};
use m3::dtu;
use m3::errors::{Code, Error};
use m3::goff;
use m3::kif::{self, boot, PEDesc, syscalls, upcalls};
use m3::server::server_loop;
use m3::session::{ResMngOperation};
use m3::util;

use childs::{BootChild, Child, Id};

const BOOT_MOD_SELS: Selector = kif::FIRST_FREE_SEL;

fn reply_result(is: &mut GateIStream, res: Result<(), Error>) {
    match res {
        Err(e) => {
            log!(ROOT, "request failed: {}", e);
            reply_vmsg!(is, e.code() as u64)
        },
        Ok(_)  => reply_vmsg!(is, 0 as u64),
    }.expect("Unable to reply");
}

fn reg_serv(is: &mut GateIStream, child: &mut Child,
            delayed: &mut Vec<BootChild>, rgate: &RecvGate,
            mods: (usize, usize)) {
    let child_sel: Selector = is.pop();
    let dst_sel: Selector = is.pop();
    let rgate_sel: Selector = is.pop();
    let name: String = is.pop();

    let res = services::get().reg_serv(child, child_sel, dst_sel, rgate_sel, name);
    if res.is_ok() && delayed.len() > 0 {
        start_delayed(delayed, mods, &rgate);
    }
    reply_result(is, res);
}

fn unreg_serv(is: &mut GateIStream, child: &mut Child) {
    let sel: Selector = is.pop();
    let notify: bool = is.pop();

    let res = services::get().unreg_serv(child, sel, notify);
    reply_result(is, res);
}

fn open_session(is: &mut GateIStream, child: &mut Child) {
    let dst_sel: Selector = is.pop();
    let name: String = is.pop();
    let arg: u64 = is.pop();

    let res = services::get().open_session(child, dst_sel, name, arg);
    reply_result(is, res);
}

fn close_session(is: &mut GateIStream, child: &mut Child) {
    let sel: Selector = is.pop();

    let res = services::get().close_session(child, sel);
    reply_result(is, res);
}

fn add_child(is: &mut GateIStream, rgate: &RecvGate, child: &mut Child) {
    let vpe_sel: Selector = is.pop();
    let sgate_sel: Selector = is.pop();
    let name: String = is.pop();

    let res = child.add_child(vpe_sel, rgate, sgate_sel, name);
    reply_result(is, res);
}

fn rem_child(is: &mut GateIStream, child: &mut Child) {
    let vpe_sel: Selector = is.pop();

    let res = child.rem_child(vpe_sel);
    reply_result(is, res);
}

fn start_delayed(delayed: &mut Vec<BootChild>, mods: (usize, usize), rgate: &RecvGate) {
    let mut idx = 0;
    while idx < delayed.len() {
        if delayed[idx].has_unmet_reqs() {
            idx += 1;
            continue;
        }

        let mut c = delayed.remove(idx);
        let mut moditer = boot::ModIterator::new(mods.0, mods.1);
        let m = moditer.nth(c.id() as usize).unwrap();
        let sel = BOOT_MOD_SELS + 1 + c.id();
        c.start(rgate, sel, &m).expect("Unable to start boot module");
        childs::get().add(Box::new(c));
    }

    if delayed.len() == 0 {
        childs::get().start_waiting(1);
    }
}

#[no_mangle]
pub fn main() -> i32 {
    let mgate = MemGate::new_bind(BOOT_MOD_SELS);
    let mut off: goff = 0;

    let info: boot::Info = mgate.read_obj(0).expect("Unable to read boot info");
    off += util::size_of::<boot::Info>() as goff;

    log!(ROOT, "BootInfo = {:?}", info);

    let mut mods_list = vec![0u8; info.mod_size as usize];
    mgate.read(&mut mods_list, off).expect("Unable to read mods");
    off += info.mod_size;

    log!(ROOT, "Boot modules:");
    let mods = (mods_list.as_slice().as_ptr() as usize, info.mod_size as usize);
    let moditer = boot::ModIterator::new(mods.0, mods.1);
    for m in moditer {
        log!(ROOT, "  {:?}", m);
    }

    let mut pes: Vec<PEDesc> = Vec::with_capacity(info.pe_count as usize);
    unsafe { pes.set_len(info.pe_count as usize) };
    mgate.read(&mut pes, off).expect("Unable to read PEs");

    let mut i = 0;
    log!(ROOT, "Available PEs:");
    for pe in pes {
        log!(
            ROOT,
            "  PE{:02}: {} {} {} KiB memory",
            i, pe.pe_type(), pe.isa(), pe.mem_size() / 1024
        );
        i += 1;
    }

    let mut rgate = RecvGate::new_with(
        RGateArgs::new().order(12).msg_order(8)
    ).expect("Unable to create RecvGate");
    rgate.activate().expect("Unable to activate RecvGate");

    let mut delayed = Vec::new();

    let moditer = boot::ModIterator::new(mods.0, mods.1);
    for (id, m) in moditer.enumerate() {
        if m.name() == "rctmux" || m.name() == "root" {
            continue;
        }

        let mut args = Vec::<String>::new();
        let mut reqs = Vec::<String>::new();
        let mut name: String = String::new();
        let mut daemon = false;
        for (idx, a) in m.name().split_whitespace().enumerate() {
            if idx == 0 {
                name = a.to_string();
                args.push(a.to_string());
            }
            else {
                if a.starts_with("requires=") {
                    reqs.push(a[9..].to_string());
                }
                else if a == "daemon" {
                    daemon = true;
                }
                else {
                    args.push(a.to_string());
                }
            }
        }

        let mut child = BootChild::new(id as Id, name, args, reqs, daemon);
        if child.reqs.len() > 0 {
            delayed.push(child);
        }
        else {
            child.start(&rgate, BOOT_MOD_SELS + 1 + id as Id, &m).expect("Unable to start boot module");
            childs::get().add(Box::new(child));
        }
    }

    if delayed.len() == 0 {
        childs::get().start_waiting(1);
    }

    let mut shutdown_inprogress = false;

    server_loop(|| {
        let is = rgate.fetch();
        if let Some(mut is) = is {
            let op: ResMngOperation = is.pop();
            let mut child = childs::get().child_by_id_mut(is.label() as Id).unwrap();

            match op {
                ResMngOperation::REG_SERV    => reg_serv(&mut is, child, &mut delayed,
                                                         &rgate, mods),
                ResMngOperation::UNREG_SERV  => unreg_serv(&mut is, child),

                ResMngOperation::OPEN_SESS   => open_session(&mut is, child),
                ResMngOperation::CLOSE_SESS  => close_session(&mut is, child),

                ResMngOperation::ADD_CHILD   => add_child(&mut is, &rgate, child),
                ResMngOperation::REM_CHILD   => rem_child(&mut is, child),

                _                            => unreachable!(),
            }
        }

        let msg = dtu::DTU::fetch_msg(RecvGate::upcall().ep().unwrap());
        if let Some(msg) = msg {
            let slice: &[upcalls::VPEWait] = unsafe { intrinsics::transmute(&msg.data) };
            let upcall = &slice[0];

            childs::get().kill_child(upcall.vpe_sel as Selector, upcall.exitcode as i32);

            let reply = syscalls::DefaultReply {
                error: 0u64,
            };
            RecvGate::upcall().reply(&[reply], msg).expect("Upcall reply failed");

            // wait for the next
            if delayed.len() == 0 {
                let no_wait_childs = childs::get().daemons() + childs::get().foreigns();
                if !shutdown_inprogress && childs::get().len() == no_wait_childs {
                    services::get().shutdown();
                    shutdown_inprogress = true;
                }
                if childs::get().len() > 0 {
                    childs::get().start_waiting(1);
                }
            }
        }

        if childs::get().len() == 0 {
            return Err(Error::new(Code::VPEGone));
        }

        Ok(())
    }).ok();

    0
}
