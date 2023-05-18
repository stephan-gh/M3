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
use base::errors::{Code, Error, VerboseError};
use base::kif::{self, syscalls};
use base::mem::MsgBuf;
use base::quota::Quota;
use base::rc::Rc;
use base::tcu;

use crate::cap::EPObject;
use crate::cap::KObject;
use crate::platform;
use crate::syscalls::{get_request, reply_success, send_reply};
use crate::tiles::{tilemng, Activity, TileMux, INVAL_ID};

#[inline(never)]
pub fn tile_quota_async(
    act: &Rc<Activity>,
    msg: &'static tcu::Message,
) -> Result<(), VerboseError> {
    let r: syscalls::TileQuota = get_request(msg)?;
    sysc_log!(act, "tile_quota(tile={})", r.tile);

    let tile = {
        let act_caps = act.obj_caps().borrow();
        get_kobj_ref!(act_caps, r.tile, Tile).clone()
    };

    let (time, pts) = if platform::tile_desc(tile.tile()).supports_tilemux() {
        TileMux::get_quota_async(
            tilemng::tilemux(tile.tile()),
            tile.time_quota_id(),
            tile.pt_quota_id(),
        )
        .map_err(|e| {
            VerboseError::new(
                e.code(),
                base::format!(
                    "Unable to get quota for time={}, pts={}",
                    tile.time_quota_id(),
                    tile.pt_quota_id()
                ),
            )
        })?
    }
    else {
        (Quota::default(), Quota::default())
    };

    let mut kreply = MsgBuf::borrow_def();
    build_vmsg!(kreply, Code::Success, kif::syscalls::TileQuotaReply {
        eps_id: tile.ep_quota().id(),
        eps_total: tile.ep_quota().total(),
        eps_left: tile.ep_quota().left(),
        time_id: time.id(),
        time_total: time.total(),
        time_left: time.remaining(),
        pts_id: pts.id(),
        pts_total: pts.total(),
        pts_left: pts.remaining(),
    });
    send_reply(msg, &kreply);

    Ok(())
}

#[inline(never)]
pub fn tile_set_quota_async(
    act: &Rc<Activity>,
    msg: &'static tcu::Message,
) -> Result<(), VerboseError> {
    let r: syscalls::TileSetQuota = get_request(msg)?;
    sysc_log!(
        act,
        "tile_set_quota(tile={}, time={}, pts={})",
        r.tile,
        r.time,
        r.pts
    );

    let tile = {
        let act_caps = act.obj_caps().borrow();
        get_kobj_ref!(act_caps, r.tile, Tile).clone()
    };

    if tile.derived() {
        sysc_err!(
            Code::NoPerm,
            "Cannot set tile quota with derived tile capability"
        );
    }
    if tile.activities() > 1 {
        sysc_err!(
            Code::InvArgs,
            "Cannot set tile quota with more than one Activity on the tile"
        );
    }

    let tilemux = tilemng::tilemux(tile.tile());

    // the root tile object has always the same id for the time quota and the pts quota
    TileMux::set_quota_async(tilemux, tile.time_quota_id(), r.time, r.pts)?;

    reply_success(msg);
    Ok(())
}

#[inline(never)]
pub fn tile_set_pmp(act: &Rc<Activity>, msg: &'static tcu::Message) -> Result<(), VerboseError> {
    let r: syscalls::TileSetPMP = get_request(msg)?;
    sysc_log!(
        act,
        "tile_set_pmp(tile={}, mgate={}, ep={}, overwrite={})",
        r.tile,
        r.mgate,
        r.ep,
        r.overwrite
    );

    let act_caps = act.obj_caps().borrow();
    let tile = get_kobj_ref!(act_caps, r.tile, Tile);
    if tile.derived() {
        sysc_err!(Code::NoPerm, "Cannot set PMP EPs for derived tile objects");
    }
    if r.overwrite && tile.activities() > 0 {
        sysc_err!(
            Code::InvState,
            "Cannot overwrite PMP EPs with existing activities"
        );
    }

    if r.ep < 1 || r.ep >= tcu::PMEM_PROT_EPS as tcu::EpId {
        sysc_err!(
            Code::InvArgs,
            "Only EPs 1..{} can be used for tile_set_pmp",
            tcu::PMEM_PROT_EPS
        );
    }

    let mut tilemux = tilemng::tilemux(tile.tile());

    // invalidate EP if requested
    if r.mgate == kif::INVALID_SEL {
        if let Err(e) = tilemux.invalidate_ep(INVAL_ID, r.ep, true, false) {
            sysc_err!(e.code(), "Unable to invalidate PMP EP");
        }
    }
    // if overwrite is disabled, the EP needs to be invalid
    else if tilemux.pmp_ep(r.ep).is_configured() && !r.overwrite {
        sysc_err!(Code::Exists, "PMP EP is already set");
    }

    // deconfigure the EP first to ensure that it is not already configured for another gate
    let ep_obj = tilemux.pmp_ep(r.ep);
    if let Err(e) = ep_obj.deconfigure(false) {
        sysc_err!(e.code(), "Unable to deconfigure PMP EP");
    }

    if r.mgate != kif::INVALID_SEL {
        let kobj = act_caps
            .get(r.mgate)
            .ok_or_else(|| Error::new(Code::InvArgs))?
            .get();
        match kobj {
            KObject::MGate(mg) => {
                if let Err(e) = tilemux.config_mem_ep(r.ep, INVAL_ID, mg, mg.tile_id()) {
                    sysc_err!(e.code(), "Unable to configure PMP EP");
                }

                // remember that the MemGate is activated on this EP for the case that the MemGate gets
                // revoked. If so, the EP is automatically invalidated.
                let ep_obj = tilemux.pmp_ep(r.ep);
                EPObject::configure(ep_obj, kobj);
            },
            _ => sysc_err!(Code::InvArgs, "Expected MemGate"),
        }
    }

    reply_success(msg);
    Ok(())
}
