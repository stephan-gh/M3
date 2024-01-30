/*
 * Copyright (C) 2019-2022 Nils Asmussen, Barkhausen Institut
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

mod chan;
mod input;

use m3::cap::SelSpace;
use m3::cell::LazyStaticRefCell;
use m3::col::Vec;
use m3::com::{opcodes, GateIStream, MemGate, Perm, RGateArgs, RecvGate};
use m3::errors::{Code, Error};
use m3::io::LogFlags;
use m3::kif;
use m3::log;
use m3::mem::GlobOff;
use m3::rc::Rc;
use m3::server::{
    server_loop, CapExchange, ClientManager, ExcType, RequestHandler, RequestSession, Server,
    ServerSession, SessId, DEF_MAX_CLIENTS,
};
use m3::tiles::Activity;

static MEM: LazyStaticRefCell<Rc<MemGate>> = LazyStaticRefCell::default();

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
enum SessionData {
    Meta,
    Chan(chan::Channel),
}

#[derive(Debug)]
pub struct VTermSession {
    _serv: ServerSession,
    data: SessionData,
    parent: Option<SessId>,
    childs: Vec<SessId>,
}

impl RequestSession for VTermSession {
    fn new(serv: ServerSession, _arg: &str) -> Result<Self, Error>
    where
        Self: Sized,
    {
        Ok(VTermSession {
            _serv: serv,
            data: SessionData::Meta,
            parent: None,
            childs: Vec::new(),
        })
    }

    fn close(&mut self, cli: &mut ClientManager<Self>, sid: SessId, sub_ids: &mut Vec<SessId>)
    where
        Self: Sized,
    {
        log!(
            LogFlags::VTReqs,
            "[{}] vterm::close(): closing {:?}",
            sid,
            sub_ids
        );

        // close child sessions as well
        sub_ids.extend_from_slice(&self.childs);

        // remove us from parent
        if let Some(pid) = self.parent.take() {
            if let Some(p) = cli.get_mut(pid) {
                p.childs.retain(|cid| *cid != sid);
            }
        }
    }
}

impl VTermSession {
    fn get_sess(cli: &mut ClientManager<Self>, sid: SessId) -> Result<&mut Self, Error> {
        cli.get_mut(sid).ok_or_else(|| Error::new(Code::InvArgs))
    }

    fn with_chan<F, R>(&mut self, is: &mut GateIStream<'_>, func: F) -> Result<R, Error>
    where
        F: Fn(&mut chan::Channel, &mut GateIStream<'_>) -> Result<R, Error>,
    {
        match &mut self.data {
            SessionData::Meta => Err(Error::new(Code::InvArgs)),
            SessionData::Chan(c) => func(c, is),
        }
    }

    fn new_chan(parent: SessId, serv: ServerSession, writing: bool) -> Result<VTermSession, Error> {
        log!(LogFlags::VTReqs, "[{}] vterm::new_chan()", serv.id());

        Ok(VTermSession {
            data: SessionData::Chan(chan::Channel::new(
                serv.id(),
                MEM.borrow().clone(),
                writing,
            )?),
            _serv: serv,
            parent: Some(parent),
            childs: Vec::new(),
        })
    }

    fn clone(
        cli: &mut ClientManager<Self>,
        crt: usize,
        sid: SessId,
        xchg: &mut CapExchange<'_>,
    ) -> Result<(), Error> {
        log!(LogFlags::VTReqs, "[{}] vterm::clone(crt={})", sid, crt);

        let (sel, _nsid) = cli.add_connected(crt, |cli, serv, _sgate| {
            let parent_sess = Self::get_sess(cli, sid)?;
            let nsid = serv.id();

            let child_sess = match &parent_sess.data {
                SessionData::Meta => {
                    let writing = xchg.in_args().pop::<i32>()? == 1;
                    Self::new_chan(sid, serv, writing)
                },

                SessionData::Chan(c) => Self::new_chan(sid, serv, c.is_writing()),
            }?;

            // remember that the new session is a child of the current one
            parent_sess.childs.push(nsid);
            Ok(child_sess)
        })?;

        xchg.out_caps(kif::CapRngDesc::new(kif::CapType::Object, sel, 2));
        Ok(())
    }

    fn set_dest(
        cli: &mut ClientManager<Self>,
        _crt: usize,
        sid: SessId,
        xchg: &mut CapExchange<'_>,
    ) -> Result<(), Error> {
        log!(LogFlags::VTReqs, "[{}] vterm::set_dest()", sid);

        let sess = Self::get_sess(cli, sid)?;
        match &mut sess.data {
            SessionData::Chan(c) => {
                let sel = SelSpace::get().alloc_sel();
                c.set_dest(sel);
                xchg.out_caps(kif::CapRngDesc::new(kif::CapType::Object, sel, 1));
                Ok(())
            },
            _ => Err(Error::new(Code::InvArgs)),
        }
    }

    fn enable_notify(
        cli: &mut ClientManager<Self>,
        _crt: usize,
        sid: SessId,
        xchg: &mut CapExchange<'_>,
    ) -> Result<(), Error> {
        log!(LogFlags::VTReqs, "[{}] vterm::set_notify()", sid);

        let sess = Self::get_sess(cli, sid)?;
        match &mut sess.data {
            SessionData::Chan(c) => {
                let rgate = RecvGate::new_with(RGateArgs::default().order(6).msg_order(6))?;
                let sgate = c.set_notify_gates(rgate)?;

                xchg.out_caps(kif::CapRngDesc::new(kif::CapType::Object, sgate, 1));
                Ok(())
            },
            _ => Err(Error::new(Code::InvArgs)),
        }
    }
}

#[no_mangle]
pub fn main() -> Result<(), Error> {
    MEM.set(Rc::new(
        MemGate::new((DEF_MAX_CLIENTS * chan::BUF_SIZE) as GlobOff, Perm::RW)
            .expect("Unable to alloc memory"),
    ));

    let mut hdl = RequestHandler::new().expect("Unable to create request handler");
    let srv = Server::new("vterm", &mut hdl).expect("Unable to create service 'vterm'");

    use opcodes::File;
    hdl.reg_cap_handler(File::CloneFile, ExcType::Obt(2), VTermSession::clone);
    hdl.reg_cap_handler(File::SetDest, ExcType::Del(1), VTermSession::set_dest);
    hdl.reg_cap_handler(
        File::EnableNotify,
        ExcType::Del(1),
        VTermSession::enable_notify,
    );

    hdl.reg_msg_handler(File::NextIn, |sess, is| {
        sess.with_chan(is, |c, is| c.next_in(is))
    });
    hdl.reg_msg_handler(File::NextOut, |sess, is| {
        sess.with_chan(is, |c, is| c.next_out(is))
    });
    hdl.reg_msg_handler(File::Commit, |sess, is| {
        sess.with_chan(is, |c, is| c.commit(is))
    });
    hdl.reg_msg_handler(File::FStat, |sess, is| {
        sess.with_chan(is, |c, is| c.stat(is))
    });
    hdl.reg_msg_handler(File::Seek, |_sess, _is| Err(Error::new(Code::NotSup)));
    hdl.reg_msg_handler(File::GetTMode, |sess, is| {
        sess.with_chan(is, |c, is| c.get_tmode(is))
    });
    hdl.reg_msg_handler(File::SetTMode, |sess, is| {
        sess.with_chan(is, |c, is| c.set_tmode(is))
    });
    hdl.reg_msg_handler(File::ReqNotify, |sess, is| {
        sess.with_chan(is, |c, is| c.request_notify(is))
    });

    let sel = SelSpace::get().alloc_sel();
    let serial_gate = Activity::own()
        .resmng()
        .unwrap()
        .get_serial(sel)
        .expect("Unable to allocate serial rgate");

    server_loop(|| {
        srv.fetch_and_handle(&mut hdl)?;

        if let Ok(msg) = serial_gate.fetch() {
            input::handle_input(hdl.clients_mut(), msg);
            serial_gate.ack_msg(msg).unwrap();
        }

        input::receive_acks(hdl.clients_mut());

        hdl.fetch_and_handle_msg();

        Ok(())
    })
    .ok();

    Ok(())
}
