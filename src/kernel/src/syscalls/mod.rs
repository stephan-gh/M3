/*
 * Copyright (C) 2020-2022 Nils Asmussen, Barkhausen Institut
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

use base::build_vmsg;
use base::errors::{Code, Error};
use base::kif;
use base::mem;
use base::rc::Rc;
use base::serialize::{Deserialize, M3Deserializer};
use base::tcu;

use crate::ktcu;
use crate::tiles::Activity;
use crate::tiles::ActivityMng;

#[macro_export]
macro_rules! sysc_log {
    ($act:expr, $fmt:tt, $($args:tt)*) => (
        klog!(
            SYSC,
            concat!("{}:{}@{}: syscall::", $fmt),
            $act.id(), $act.name(), $act.tile_id(), $($args)*
        )
    )
}

#[macro_export]
macro_rules! sysc_err {
    ($e:expr, $fmt:tt) => ({
        return Err(base::errors::VerboseError::new($e, $fmt.to_string()));
    });
    ($e:expr, $fmt:tt, $($args:tt)*) => ({
        return Err(base::errors::VerboseError::new($e, base::format!($fmt, $($args)*)));
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
    ($act:expr, $sel:expr, $ty:ident) => {{
        let kobj = get_cap!($act.obj_caps().borrow(), $sel).get().clone();
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

fn send_reply(msg: &'static tcu::Message, rep: &mem::MsgBuf) {
    ktcu::reply(ktcu::KSYS_EP, rep, msg).ok();
}

fn reply_result(msg: &'static tcu::Message, error: Code) {
    let mut rep_buf = mem::MsgBuf::borrow_def();
    build_vmsg!(rep_buf, kif::DefaultReply { error });
    send_reply(msg, &rep_buf);
}

fn reply_success(msg: &'static tcu::Message) {
    reply_result(msg, Code::Success);
}

fn get_request<R: Deserialize<'static>>(msg: &'static tcu::Message) -> Result<R, Error> {
    let mut de = M3Deserializer::new(msg.as_words());
    de.skip(1);
    de.pop()
}

pub fn handle_async(msg: &'static tcu::Message) {
    let act: Rc<Activity> = ActivityMng::activity(msg.header.label() as tcu::ActId).unwrap();

    let opcode = msg.as_words()[0];
    let op = kif::syscalls::Operation::from(opcode);
    let res = match op {
        kif::syscalls::Operation::CREATE_MGATE => create::create_mgate(&act, msg),
        kif::syscalls::Operation::CREATE_RGATE => create::create_rgate(&act, msg),
        kif::syscalls::Operation::CREATE_SGATE => create::create_sgate(&act, msg),
        kif::syscalls::Operation::CREATE_SRV => create::create_srv(&act, msg),
        kif::syscalls::Operation::CREATE_SESS => create::create_sess(&act, msg),
        kif::syscalls::Operation::CREATE_ACT => create::create_activity_async(&act, msg),
        kif::syscalls::Operation::CREATE_SEM => create::create_sem(&act, msg),
        kif::syscalls::Operation::CREATE_MAP => create::create_map_async(&act, msg),

        kif::syscalls::Operation::DERIVE_TILE => derive::derive_tile_async(&act, msg),
        kif::syscalls::Operation::DERIVE_MEM => derive::derive_mem(&act, msg),
        kif::syscalls::Operation::DERIVE_KMEM => derive::derive_kmem(&act, msg),
        kif::syscalls::Operation::DERIVE_SRV => derive::derive_srv_async(&act, msg),

        kif::syscalls::Operation::EXCHANGE => exchange::exchange(&act, msg),
        kif::syscalls::Operation::EXCHANGE_SESS => exchange::exchange_over_sess_async(&act, msg),
        kif::syscalls::Operation::REVOKE => exchange::revoke_async(&act, msg),

        kif::syscalls::Operation::ALLOC_EP => misc::alloc_ep(&act, msg),
        kif::syscalls::Operation::SET_PMP => misc::set_pmp(&act, msg),
        kif::syscalls::Operation::ACTIVATE => misc::activate_async(&act, msg),
        kif::syscalls::Operation::MGATE_REGION => misc::mgate_region(&act, msg),
        kif::syscalls::Operation::RGATE_BUFFER => misc::rgate_buffer(&act, msg),
        kif::syscalls::Operation::KMEM_QUOTA => misc::kmem_quota(&act, msg),
        kif::syscalls::Operation::TILE_QUOTA => misc::tile_quota_async(&act, msg),
        kif::syscalls::Operation::TILE_SET_QUOTA => misc::tile_set_quota_async(&act, msg),
        kif::syscalls::Operation::GET_SESS => misc::get_sess(&act, msg),
        kif::syscalls::Operation::SEM_CTRL => misc::sem_ctrl_async(&act, msg),
        kif::syscalls::Operation::ACT_CTRL => misc::activity_ctrl_async(&act, msg),
        kif::syscalls::Operation::ACT_WAIT => misc::activity_wait_async(&act, msg),

        kif::syscalls::Operation::RESET_STATS => misc::reset_stats(&act, msg),
        kif::syscalls::Operation::NOOP => misc::noop(&act, msg),

        _ => panic!("Unexpected operation: {}", opcode),
    };

    if let Err(e) = res {
        klog!(
            ERR,
            "\x1B[37;41m{}:{}@{}: {:?} failed: {} ({:?})\x1B[0m",
            act.id(),
            act.name(),
            act.tile_id(),
            op,
            e.msg(),
            e.code()
        );

        reply_result(msg, e.code());
    }
}
