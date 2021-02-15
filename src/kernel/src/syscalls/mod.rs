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

use base::col::String;
use base::errors::{Code, Error};
use base::kif::{self};
use base::mem;
use base::rc::Rc;
use base::tcu;

use crate::ktcu;
use crate::pes::VPEMng;
use crate::pes::VPE;

#[macro_export]
macro_rules! sysc_log {
    ($vpe:expr, $fmt:tt, $($args:tt)*) => (
        klog!(
            SYSC,
            concat!("{}:{}@{}: syscall::", $fmt),
            $vpe.id(), $vpe.name(), $vpe.pe_id(), $($args)*
        )
    )
}

#[macro_export]
macro_rules! sysc_err {
    ($e:expr, $fmt:tt) => ({
        return Err(SyscError::new($e, $fmt.to_string()));
    });
    ($e:expr, $fmt:tt, $($args:tt)*) => ({
        return Err(SyscError::new($e, base::format!($fmt, $($args)*)));
    });
}

macro_rules! try_kmem_quota {
    ($e:expr) => {
        if let Err(e) = $e {
            sysc_err!(e.code(), "Insufficient kernel memory quota");
        }
    };
}

macro_rules! as_obj {
    ($kobj:expr, $ty:ident) => {
        match $kobj {
            KObject::$ty(k) => k,
            _ => sysc_err!(Code::InvArgs, "Expected {:?} cap", stringify!($ty)),
        }
    };
}
macro_rules! get_cap {
    ($table:expr, $sel:expr) => {{
        // note that we deliberately use match here, because ok_or_else(...)? results in worse code
        match $table.get($sel) {
            Some(c) => c,
            None => sysc_err!(Code::InvArgs, "Invalid capability"),
        }
    }};
}
macro_rules! get_kobj {
    ($vpe:expr, $sel:expr, $ty:ident) => {{
        let kobj = get_cap!($vpe.obj_caps().borrow(), $sel).get().clone();
        as_obj!(kobj, $ty)
    }};
}
macro_rules! get_kobj_ref {
    ($table:expr, $sel:expr, $ty:ident) => {{
        let cap = get_cap!($table, $sel);
        as_obj!(cap.get(), $ty)
    }};
}

mod create;
mod derive;
mod exchange;
mod misc;

pub struct SyscError {
    pub code: Code,
    pub msg: String,
}

impl SyscError {
    pub fn new(code: Code, msg: String) -> Self {
        SyscError { code, msg }
    }
}

impl From<Error> for SyscError {
    fn from(e: Error) -> Self {
        SyscError::new(e.code(), String::default())
    }
}

fn send_reply<T>(msg: &'static tcu::Message, rep: &T) {
    ktcu::reply(ktcu::KSYS_EP, rep, msg).ok();
}

fn reply_result(msg: &'static tcu::Message, code: u64) {
    let rep = kif::DefaultReply { error: code };
    send_reply(msg, &rep);
}

fn reply_success(msg: &'static tcu::Message) {
    reply_result(msg, 0);
}

fn get_request<R>(msg: &tcu::Message) -> Result<&R, Error> {
    if msg.data.len() < mem::size_of::<R>() {
        Err(Error::new(Code::InvArgs))
    }
    else {
        Ok(msg.get_data())
    }
}

pub fn handle_async(msg: &'static tcu::Message) {
    let vpe: Rc<VPE> = VPEMng::get().vpe(msg.header.label as tcu::VPEId).unwrap();
    let req = msg.get_data::<kif::DefaultRequest>();

    let res = match kif::syscalls::Operation::from(req.opcode) {
        kif::syscalls::Operation::CREATE_MGATE => create::create_mgate(&vpe, msg),
        kif::syscalls::Operation::CREATE_RGATE => create::create_rgate(&vpe, msg),
        kif::syscalls::Operation::CREATE_SGATE => create::create_sgate(&vpe, msg),
        kif::syscalls::Operation::CREATE_SRV => create::create_srv(&vpe, msg),
        kif::syscalls::Operation::CREATE_SESS => create::create_sess(&vpe, msg),
        kif::syscalls::Operation::CREATE_VPE => create::create_vpe_async(&vpe, msg),
        kif::syscalls::Operation::CREATE_SEM => create::create_sem(&vpe, msg),
        kif::syscalls::Operation::CREATE_MAP => create::create_map_async(&vpe, msg),

        kif::syscalls::Operation::DERIVE_PE => derive::derive_pe(&vpe, msg),
        kif::syscalls::Operation::DERIVE_MEM => derive::derive_mem(&vpe, msg),
        kif::syscalls::Operation::DERIVE_KMEM => derive::derive_kmem(&vpe, msg),
        kif::syscalls::Operation::DERIVE_SRV => derive::derive_srv_async(&vpe, msg),

        kif::syscalls::Operation::EXCHANGE => exchange::exchange(&vpe, msg),
        kif::syscalls::Operation::DELEGATE => exchange::exchange_over_sess_async(&vpe, msg, false),
        kif::syscalls::Operation::OBTAIN => exchange::exchange_over_sess_async(&vpe, msg, true),
        kif::syscalls::Operation::REVOKE => exchange::revoke_async(&vpe, msg),

        kif::syscalls::Operation::ALLOC_EP => misc::alloc_ep(&vpe, msg),
        kif::syscalls::Operation::SET_PMP => misc::set_pmp(&vpe, msg),
        kif::syscalls::Operation::ACTIVATE => misc::activate_async(&vpe, msg),
        kif::syscalls::Operation::KMEM_QUOTA => misc::kmem_quota(&vpe, msg),
        kif::syscalls::Operation::PE_QUOTA => misc::pe_quota(&vpe, msg),
        kif::syscalls::Operation::GET_SESS => misc::get_sess(&vpe, msg),
        kif::syscalls::Operation::SEM_CTRL => misc::sem_ctrl_async(&vpe, msg),
        kif::syscalls::Operation::VPE_CTRL => misc::vpe_ctrl_async(&vpe, msg),
        kif::syscalls::Operation::VPE_WAIT => misc::vpe_wait_async(&vpe, msg),

        kif::syscalls::Operation::NOOP => misc::noop(&vpe, msg),

        _ => panic!("Unexpected operation: {}", { req.opcode }),
    };

    if let Err(e) = res {
        klog!(
            ERR,
            "\x1B[37;41m{}:{}@{}: {:?} failed: {} ({:?})\x1B[0m",
            vpe.id(),
            vpe.name(),
            vpe.pe_id(),
            kif::syscalls::Operation::from(req.opcode),
            e.msg,
            e.code
        );

        reply_result(msg, e.code as u64);
    }
}
