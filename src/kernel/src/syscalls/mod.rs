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

use core::convert::TryFrom;

use base::build_vmsg;
use base::errors::{Code, Error};
use base::format;
use base::io::LogFlags;
use base::kif;
use base::log;
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
        $crate::log!(
            base::io::LogFlags::KernSysc,
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
    let op = kif::syscalls::Operation::try_from(opcode)
        .expect(&format!("Unexpected operation {}", opcode));
    let res = match op {
        kif::syscalls::Operation::CreateMGate => create::create_mgate(&act, msg),
        kif::syscalls::Operation::CreateRGate => create::create_rgate(&act, msg),
        kif::syscalls::Operation::CreateSGate => create::create_sgate(&act, msg),
        kif::syscalls::Operation::CreateSrv => create::create_srv(&act, msg),
        kif::syscalls::Operation::CreateSess => create::create_sess(&act, msg),
        kif::syscalls::Operation::CreateAct => create::create_activity_async(&act, msg),
        kif::syscalls::Operation::CreateSem => create::create_sem(&act, msg),
        kif::syscalls::Operation::CreateMap => create::create_map_async(&act, msg),

        kif::syscalls::Operation::DeriveTile => derive::derive_tile_async(&act, msg),
        kif::syscalls::Operation::DeriveMem => derive::derive_mem(&act, msg),
        kif::syscalls::Operation::DeriveKMem => derive::derive_kmem(&act, msg),
        kif::syscalls::Operation::DeriveSrv => derive::derive_srv_async(&act, msg),

        kif::syscalls::Operation::Exchange => exchange::exchange(&act, msg),
        kif::syscalls::Operation::ExchangeSess => exchange::exchange_over_sess_async(&act, msg),
        kif::syscalls::Operation::Revoke => exchange::revoke_async(&act, msg),

        kif::syscalls::Operation::AllocEP => misc::alloc_ep(&act, msg),
        kif::syscalls::Operation::SetPMP => misc::set_pmp(&act, msg),
        kif::syscalls::Operation::Activate => misc::activate_async(&act, msg),
        kif::syscalls::Operation::MGateRegion => misc::mgate_region(&act, msg),
        kif::syscalls::Operation::RGateBuffer => misc::rgate_buffer(&act, msg),
        kif::syscalls::Operation::KMemQuota => misc::kmem_quota(&act, msg),
        kif::syscalls::Operation::TileQuota => misc::tile_quota_async(&act, msg),
        kif::syscalls::Operation::TileSetQuota => misc::tile_set_quota_async(&act, msg),
        kif::syscalls::Operation::GetSess => misc::get_sess(&act, msg),
        kif::syscalls::Operation::SemCtrl => misc::sem_ctrl_async(&act, msg),
        kif::syscalls::Operation::ActCtrl => misc::activity_ctrl_async(&act, msg),
        kif::syscalls::Operation::ActWait => misc::activity_wait_async(&act, msg),

        kif::syscalls::Operation::ResetStats => misc::reset_stats(&act, msg),
        kif::syscalls::Operation::Noop => misc::noop(&act, msg),
    };

    if let Err(e) = res {
        log!(
            LogFlags::Error,
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
