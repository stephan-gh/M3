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

use base::col::ToString;
use base::errors::{Code, VerboseError};
use base::format;
use base::kif::{service, syscalls, CapRngDesc, CapSel, CapType, SEL_ACT};
use base::mem::MsgBuf;
use base::rc::Rc;
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
            format!("Cap types differ ({} vs {})", c1.cap_type(), c2.cap_type()),
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
    let req: &syscalls::Exchange = get_request(msg)?;
    let act_sel = req.act_sel as CapSel;
    let own_crd = CapRngDesc::new_from(req.own_caps);
    let other_crd = CapRngDesc::new(own_crd.cap_type(), req.other_sel as CapSel, own_crd.count());
    let obtain = req.obtain == 1;

    sysc_log!(
        act,
        "exchange(act={}, own={}, other={}, obtain={})",
        act_sel,
        own_crd,
        other_crd,
        obtain
    );

    let actcap = get_kobj!(act, act_sel, Activity).upgrade().unwrap();
    do_exchange(act, &actcap, &own_crd, &other_crd, obtain)?;

    reply_success(msg);
    Ok(())
}

#[inline(never)]
pub fn exchange_over_sess_async(
    act: &Rc<Activity>,
    msg: &'static tcu::Message,
    obtain: bool,
) -> Result<(), VerboseError> {
    let req: &syscalls::ExchangeSess = get_request(msg)?;
    let act_sel = req.act_sel as CapSel;
    let sess_sel = req.sess_sel as CapSel;
    let crd = CapRngDesc::new_from(req.caps);

    let name = if obtain { "obtain" } else { "delegate" };
    sysc_log!(
        act,
        "{}(act={}, sess={}, crd={})",
        name,
        act_sel,
        sess_sel,
        crd
    );

    let actcap = get_kobj!(act, act_sel, Activity).upgrade().unwrap();
    let sess = get_kobj!(act, sess_sel, Sess);

    let mut smsg = MsgBuf::borrow_def();
    let data = service::ExchangeData {
        caps: crd,
        args: req.args,
    };
    build_vmsg!(
        smsg,
        if obtain {
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

    klog!(
        SERV,
        "Sending {}(sess={:#x}, caps={}, args={}B) to service {} with creator {}",
        name,
        sess.ident(),
        crd.count(),
        { req.args.bytes },
        serv.service().name(),
        label,
    );
    let rmsg = match Service::send_receive_async(serv.service(), label, smsg) {
        Ok(rmsg) => rmsg,
        Err(e) => sysc_err!(e.code(), "Service {} unreachable", serv.service().name()),
    };

    match *get_request::<u64>(rmsg)? {
        0 => {},
        err => sysc_err!(
            Code::from(err as u32),
            "Server {} denied cap exchange",
            serv.service().name()
        ),
    }

    let reply: &service::ExchangeReply = get_request(rmsg)?;

    sysc_log!(
        act,
        "{} continue with res={:?}, srv_crd={}",
        name,
        reply.res,
        reply.data.caps
    );

    do_exchange(
        &actcap,
        &serv.service().activity(),
        &crd,
        &reply.data.caps,
        obtain,
    )?;

    let mut kreply = MsgBuf::borrow_def();
    kreply.set(syscalls::ExchangeSessReply {
        error: 0,
        args: reply.data.args,
    });
    send_reply(msg, &kreply);

    Ok(())
}

#[inline(never)]
pub fn revoke_async(act: &Rc<Activity>, msg: &'static tcu::Message) -> Result<(), VerboseError> {
    let req: &syscalls::Revoke = get_request(msg)?;
    let act_sel = req.act_sel as CapSel;
    let crd = CapRngDesc::new_from(req.caps);
    let own = req.own == 1;

    sysc_log!(act, "revoke(act={}, crd={}, own={})", act_sel, crd, own);

    if crd.cap_type() == CapType::OBJECT && crd.start() <= SEL_ACT {
        sysc_err!(Code::InvArgs, "Cap 0, 1, and 2 are not revokeable");
    }

    let actcap = get_kobj!(act, act_sel, Activity).upgrade().unwrap();
    if let Err(e) = actcap.revoke_async(crd, own) {
        sysc_err!(
            e.code(),
            "Revoke of {} with Activity {} failed",
            crd,
            act.id()
        );
    }

    reply_success(msg);
    Ok(())
}
