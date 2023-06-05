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
use base::col::ToString;
use base::errors::{Code, VerboseError};
use base::format;
use base::io::LogFlags;
use base::kif::{service, syscalls, CapRngDesc, CapType, SEL_ACT};
use base::log;
use base::mem::MsgBuf;
use base::rc::Rc;
use base::serialize::M3Deserializer;
use base::tcu;

use crate::cap::KObject;
use crate::com::Service;
use crate::syscalls::{get_request, reply_success, send_reply};
use crate::tiles::Activity;

fn do_exchange(
    act1: &Rc<Activity>,
    act2: &Rc<Activity>,
    c1: &CapRngDesc,
    c2: &CapRngDesc,
    obtain: bool,
) -> Result<(), VerboseError> {
    let src = if obtain { act2 } else { act1 };
    let dst = if obtain { act1 } else { act2 };
    let src_rng = if obtain { c2 } else { c1 };
    let dst_rng = if obtain { c1 } else { c2 };

    if act1.id() == act2.id() {
        return Err(VerboseError::new(
            Code::InvArgs,
            "Cannot exchange with same Activity".to_string(),
        ));
    }
    if c1.cap_type() != c2.cap_type() {
        return Err(VerboseError::new(
            Code::InvArgs,
            format!(
                "Cap types differ ({:?} vs {:?})",
                c1.cap_type(),
                c2.cap_type()
            ),
        ));
    }
    if (obtain && c2.count() > c1.count()) || (!obtain && c2.count() != c1.count()) {
        return Err(VerboseError::new(
            Code::InvArgs,
            format!("Cap counts differ ({} vs {})", c2.count(), c1.count()),
        ));
    }
    if !dst.obj_caps().borrow().range_unused(dst_rng) {
        return Err(VerboseError::new(
            Code::InvArgs,
            "Destination selectors already in use".to_string(),
        ));
    }

    for i in 0..c2.count() {
        let src_sel = src_rng.start() + i;
        let dst_sel = dst_rng.start() + i;
        let mut obj_caps_ref = src.obj_caps().borrow_mut();
        let src_cap = obj_caps_ref.get_mut(src_sel);
        src_cap.map(|c| dst.obj_caps().borrow_mut().obtain(dst_sel, c, true));
    }

    Ok(())
}

#[inline(never)]
pub fn exchange(act: &Rc<Activity>, msg: &'static tcu::Message) -> Result<(), VerboseError> {
    let r: syscalls::Exchange = get_request(msg)?;
    let other_crd = CapRngDesc::new(r.own.cap_type(), r.other, r.own.count());

    sysc_log!(
        act,
        "exchange(act={}, own={}, other={}, obtain={})",
        r.act,
        r.own,
        other_crd,
        r.obtain
    );

    let actcap = get_kobj!(act, r.act, Activity).upgrade().unwrap();
    do_exchange(act, &actcap, &r.own, &other_crd, r.obtain)?;

    reply_success(msg);
    Ok(())
}

#[inline(never)]
pub fn exchange_over_sess_async(
    act: &Rc<Activity>,
    msg: &'static tcu::Message,
) -> Result<(), VerboseError> {
    let r: syscalls::ExchangeSess = get_request(msg)?;
    let name = if r.obtain { "obtain" } else { "delegate" };
    sysc_log!(
        act,
        "{}(act={}, sess={}, crd={})",
        name,
        r.act,
        r.sess,
        r.crd
    );

    let actcap = get_kobj!(act, r.act, Activity).upgrade().unwrap();
    let sess = get_kobj!(act, r.sess, Sess);

    let mut smsg = MsgBuf::borrow_def();
    let data = service::ExchangeData {
        caps: r.crd,
        args: r.args,
    };
    build_vmsg!(
        smsg,
        if r.obtain {
            service::Request::Obtain {
                sid: sess.ident(),
                data,
            }
        }
        else {
            service::Request::Delegate {
                sid: sess.ident(),
                data,
            }
        }
    );

    let serv = sess.service().clone();
    let label = sess.creator() as tcu::Label;

    log!(
        LogFlags::KernServ,
        "Sending {}(sess={:#x}, caps={}, args={}B) to service {} with creator {}",
        name,
        sess.ident(),
        r.crd.count(),
        r.args.bytes,
        serv.service().name(),
        label,
    );
    let rmsg = match Service::send_receive_async(serv.service(), label, smsg) {
        Ok(rmsg) => rmsg,
        Err(e) => sysc_err!(e.code(), "Service {} unreachable", serv.service().name()),
    };

    let mut de = M3Deserializer::new(rmsg.as_words());
    let err: Code = de.pop()?;
    match err {
        Code::Success => {},
        err => sysc_err!(err, "Server {} denied cap exchange", serv.service().name()),
    }

    let reply: service::ExchangeReply = de.pop()?;

    sysc_log!(
        act,
        "{} continue with res={:?}, srv_crd={}",
        name,
        err,
        reply.data.caps
    );

    do_exchange(
        &actcap,
        &serv.service().activity(),
        &r.crd,
        &reply.data.caps,
        r.obtain,
    )?;

    let mut kreply = MsgBuf::borrow_def();
    build_vmsg!(kreply, Code::Success, syscalls::ExchangeSessReply {
        args: reply.data.args,
    });
    send_reply(msg, &kreply);

    Ok(())
}

#[inline(never)]
pub fn revoke_async(act: &Rc<Activity>, msg: &'static tcu::Message) -> Result<(), VerboseError> {
    let r: syscalls::Revoke = get_request(msg)?;
    sysc_log!(act, "revoke(act={}, crd={}, own={})", r.act, r.crd, r.own);

    if r.crd.cap_type() == CapType::Object && r.crd.start() <= SEL_ACT {
        sysc_err!(Code::InvArgs, "Cap 0, 1, and 2 are not revokeable");
    }

    let actcap = get_kobj!(act, r.act, Activity).upgrade().unwrap();
    if let Err(e) = actcap.revoke_async(r.crd, r.own, act.id()) {
        sysc_err!(
            e.code(),
            "Revoke of {} with Activity {} failed",
            r.crd,
            act.id()
        );
    }

    reply_success(msg);
    Ok(())
}
