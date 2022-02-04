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

use base::col::ToString;
use base::errors::{Code, VerboseError};
use base::goff;
use base::kif::{self, CapRngDesc, CapSel, CapType};
use base::mem::{GlobAddr, MsgBuf};
use base::rc::Rc;
use base::tcu;

use crate::cap::{Capability, KObject};
use crate::cap::{EPQuota, KMemObject, MGateObject, ServObject, TileObject};
use crate::com::Service;
use crate::mem;
use crate::syscalls::{get_request, reply_success};
use crate::tiles::{tilemng, Activity, TileMux};

#[inline(never)]
pub fn derive_tile_async(
    act: &Rc<Activity>,
    msg: &'static tcu::Message,
) -> Result<(), VerboseError> {
    let req: &kif::syscalls::DeriveTile = get_request(msg)?;
    let tile_sel = req.tile_sel as CapSel;
    let dst_sel = req.dst_sel as CapSel;
    let eps = req.eps.get();
    let time = req.time.get();
    let pts = req.pts.get();

    sysc_log!(
        act,
        "derive_tile(tile={}, dst={}, eps={:?}, time={:?}, pts={:?})",
        tile_sel,
        dst_sel,
        eps,
        time,
        pts,
    );

    if !act.obj_caps().borrow().unused(dst_sel) {
        sysc_err!(Code::InvArgs, "Selector {} already in use", dst_sel);
    }

    let tile = get_kobj!(act, tile_sel, Tile);

    let ep_quota = if let Some(eps) = eps {
        if !tile.has_quota(eps) {
            sysc_err!(Code::NoSpace, "Insufficient EPs");
        }
        tile.alloc(eps);

        EPQuota::new(eps)
    }
    else {
        tile.ep_quota().clone()
    };

    let (time_id, pt_id) = if time.is_some() || pts.is_some() {
        let tilemux = tilemng::tilemux(tile.tile());
        match TileMux::derive_quota_async(
            tilemux,
            tile.time_quota_id(),
            tile.pt_quota_id(),
            time,
            pts,
        ) {
            Err(e) => {
                if let Some(eps) = eps {
                    tile.free(eps);
                }
                return Err(VerboseError::from(e));
            },
            Ok(v) => v,
        }
    }
    else {
        (tile.time_quota_id(), tile.pt_quota_id())
    };

    let cap = Capability::new(
        dst_sel,
        KObject::Tile(TileObject::new(tile.tile(), ep_quota, time_id, pt_id, true)),
    );
    // TODO we will leak the quota object in TileMux if this fails
    try_kmem_quota!(act.obj_caps().borrow_mut().insert_as_child(cap, tile_sel));

    reply_success(msg);
    Ok(())
}

#[inline(never)]
pub fn derive_kmem(act: &Rc<Activity>, msg: &'static tcu::Message) -> Result<(), VerboseError> {
    let req: &kif::syscalls::DeriveKMem = get_request(msg)?;
    let kmem_sel = req.kmem_sel as CapSel;
    let dst_sel = req.dst_sel as CapSel;
    let quota = req.quota as usize;

    sysc_log!(
        act,
        "derive_kmem(kmem={}, dst={}, quota={:#x})",
        kmem_sel,
        dst_sel,
        quota
    );

    if !act.obj_caps().borrow().unused(dst_sel) {
        sysc_err!(Code::InvArgs, "Selector {} already in use", dst_sel);
    }

    let kmem = get_kobj!(act, kmem_sel, KMem);
    if !kmem.has_quota(quota) {
        sysc_err!(Code::NoSpace, "Insufficient quota");
    }

    let cap = Capability::new(dst_sel, KObject::KMem(KMemObject::new(quota)));
    try_kmem_quota!(act.obj_caps().borrow_mut().insert_as_child(cap, kmem_sel));
    assert!(kmem.alloc(act, kmem_sel, quota));

    reply_success(msg);
    Ok(())
}

#[inline(never)]
pub fn derive_mem(act: &Rc<Activity>, msg: &'static tcu::Message) -> Result<(), VerboseError> {
    let req: &kif::syscalls::DeriveMem = get_request(msg)?;
    let act_sel = req.act_sel as CapSel;
    let dst_sel = req.dst_sel as CapSel;
    let src_sel = req.src_sel as CapSel;
    let offset = req.offset as goff;
    let size = req.size as goff;
    let perms = kif::Perm::from_bits_truncate(req.perms as u32);

    sysc_log!(
        act,
        "derive_mem(act={}, src={}, dst={}, size={:#x}, offset={:#x}, perms={:?})",
        act_sel,
        src_sel,
        dst_sel,
        size,
        offset,
        perms
    );

    let tact = get_kobj!(act, act_sel, Activity).upgrade().unwrap();
    if !tact.obj_caps().borrow().unused(dst_sel) {
        sysc_err!(Code::InvArgs, "Selector {} already in use", dst_sel);
    }

    let cap = {
        let act_caps = act.obj_caps().borrow();
        let mgate = get_kobj_ref!(act_caps, src_sel, MGate);
        if offset.checked_add(size).is_none() || offset + size > mgate.size() || size == 0 {
            sysc_err!(Code::InvArgs, "Size or offset invalid");
        }

        let addr = mgate.addr().raw() + offset as u64;
        let new_mem = mem::Allocation::new(GlobAddr::new(addr), size);
        let mgate_obj = MGateObject::new(new_mem, perms & mgate.perms(), true);
        Capability::new(dst_sel, KObject::MGate(mgate_obj))
    };

    try_kmem_quota!(tact.obj_caps().borrow_mut().insert_as_child(cap, src_sel));

    reply_success(msg);
    Ok(())
}

#[inline(never)]
pub fn derive_srv_async(
    act: &Rc<Activity>,
    msg: &'static tcu::Message,
) -> Result<(), VerboseError> {
    let req: &kif::syscalls::DeriveSrv = get_request(msg)?;
    let dst_crd = CapRngDesc::new(CapType::OBJECT, req.dst_sel, 2);
    let srv_sel = req.srv_sel as CapSel;
    let sessions = req.sessions as u32;
    let event = req.event;

    sysc_log!(
        act,
        "derive_srv(dst={}, srv={}, sessions={}, event={})",
        dst_crd,
        srv_sel,
        sessions,
        event
    );

    if !act.obj_caps().borrow().range_unused(&dst_crd) {
        sysc_err!(Code::InvArgs, "Selectors {} already in use", dst_crd);
    }
    if sessions == 0 {
        sysc_err!(Code::InvArgs, "Invalid session count");
    }

    let srvcap = get_kobj!(act, srv_sel, Serv);

    // everything worked, send the reply
    reply_success(msg);

    let mut smsg = MsgBuf::borrow_def();
    smsg.set(kif::service::DeriveCreator {
        opcode: kif::service::Operation::DERIVE_CRT.val as u64,
        sessions: sessions as u64,
    });

    let label = srvcap.creator() as tcu::Label;
    klog!(
        SERV,
        "Sending DERIVE_CRT(sessions={}) to service {} with creator {}",
        sessions,
        srvcap.service().name(),
        label,
    );
    let res = Service::send_receive_async(srvcap.service(), label, smsg);

    let res = match res {
        Err(e) => {
            sysc_log!(
                act,
                "Service {} unreachable: {:?}",
                srvcap.service().name(),
                e.code()
            );
            Err(e)
        },

        Ok(rmsg) => {
            match Result::from(Code::from(*get_request::<u64>(rmsg)? as u32)) {
                Err(e) => {
                    sysc_log!(
                        act,
                        "Server {} denied derive: {:?}",
                        srvcap.service().name(),
                        e.code()
                    );
                    Err(e)
                },
                Ok(_) => {
                    let reply: &kif::service::DeriveCreatorReply = get_request(rmsg)?;
                    let creator = reply.creator as usize;
                    let sgate_sel = reply.sgate_sel as CapSel;

                    sysc_log!(act, "derive_srv continue with creator={}", creator);

                    // obtain SendGate from server (do that first because it can fail)
                    let serv_act = srvcap.service().activity();
                    let mut serv_caps = serv_act.obj_caps().borrow_mut();
                    let src_cap = serv_caps.get_mut(sgate_sel);
                    match src_cap {
                        None => sysc_log!(act, "Service gave invalid SendGate cap {}", sgate_sel),
                        Some(c) => try_kmem_quota!(act.obj_caps().borrow_mut().obtain(
                            dst_crd.start() + 1,
                            c,
                            true
                        )),
                    }

                    // derive new service object
                    let cap = Capability::new(
                        dst_crd.start() + 0,
                        KObject::Serv(ServObject::new(srvcap.service().clone(), false, creator)),
                    );
                    try_kmem_quota!(act.obj_caps().borrow_mut().insert_as_child(cap, srv_sel));
                    Ok(())
                },
            }
        },
    };

    act.upcall_derive_srv(event, res);
    Ok(())
}
