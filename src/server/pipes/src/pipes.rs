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
#![feature(vec_remove_item)]

#[macro_use]
extern crate m3;
#[macro_use]
extern crate bitflags;

mod sess;

use m3::cap::Selector;
use m3::cell::LazyStaticCell;
use m3::com::GateIStream;
use m3::errors::{Code, Error};
use m3::kif;
use m3::pes::VPE;
use m3::serialize::Source;
use m3::server::{
    server_loop, CapExchange, Handler, RequestHandler, Server, SessId, SessionContainer,
    DEF_MAX_CLIENTS,
};
use m3::session::ServerSession;
use m3::tcu::Label;
use m3::vfs::GenFileOp;

use sess::{ChanType, Channel, Meta, PipesSession, SessionData};

pub const LOG_DEF: bool = false;

static REQHDL: LazyStaticCell<RequestHandler> = LazyStaticCell::default();

struct PipesHandler {
    sel: Selector,
    sessions: SessionContainer<PipesSession>,
}

impl PipesHandler {
    fn new_sub_sess(
        &self,
        crt: usize,
        sel: Selector,
        nsid: SessId,
        data: SessionData,
    ) -> Result<PipesSession, Error> {
        // let the kernel close the session as soon as the client dies or the session is revoked
        // for some other reason. this is required to signal EOF to the other side of the pipe.
        Ok(PipesSession::new(
            crt,
            ServerSession::new_with_sel(self.sel, sel, crt, nsid as u64, true)?,
            data,
        ))
    }

    fn close_sess(&mut self, sid: SessId) -> Result<(), Error> {
        // close this and all child sessions
        let mut sids = vec![sid];
        while let Some(id) = sids.pop() {
            if let Some(sess) = self.sessions.get_mut(id) {
                log!(crate::LOG_DEF, "[{}] pipes::close(): closing {}", sid, id);

                // ignore errors here
                let _ = match &mut sess.data_mut() {
                    SessionData::Meta(ref mut m) => m.close(&mut sids),
                    SessionData::Pipe(ref mut p) => p.close(&mut sids),
                    SessionData::Chan(ref mut c) => c.close(&mut sids),
                };

                let crt = sess.creator();
                self.sessions.remove(crt, id);
                // ignore all potentially outstanding messages of this session
                REQHDL.recv_gate().drop_msgs_with(id as Label);
            }
        }
        Ok(())
    }

    fn with_chan<F, R>(&mut self, is: &mut GateIStream, func: F) -> Result<R, Error>
    where
        F: Fn(&mut Channel, &mut GateIStream) -> Result<R, Error>,
    {
        let sess = self.sessions.get_mut(is.label() as SessId).unwrap();
        match &mut sess.data_mut() {
            SessionData::Chan(c) => func(c, is),
            _ => Err(Error::new(Code::InvArgs)),
        }
    }
}

impl Handler<PipesSession> for PipesHandler {
    fn sessions(&mut self) -> &mut m3::server::SessionContainer<PipesSession> {
        &mut self.sessions
    }

    fn open(
        &mut self,
        crt: usize,
        srv_sel: Selector,
        _arg: &str,
    ) -> Result<(Selector, SessId), Error> {
        self.sessions.add_next(crt, srv_sel, true, |sess| {
            log!(crate::LOG_DEF, "[{}] pipes::new_meta()", sess.ident());
            Ok(PipesSession::new(
                crt,
                sess,
                SessionData::Meta(Meta::default()),
            ))
        })
    }

    fn obtain(&mut self, crt: usize, sid: SessId, xchg: &mut CapExchange) -> Result<(), Error> {
        if xchg.in_caps() != 2 {
            return Err(Error::new(Code::InvArgs));
        }
        if !self.sessions.can_add(crt) {
            return Err(Error::new(Code::NoSpace));
        }

        let res: Result<_, Error> = {
            let nsid = self.sessions.next_id()?;
            let osess = self.sessions.get_mut(sid).unwrap();
            match &mut osess.data_mut() {
                // meta sessions allow to create new pipes
                SessionData::Meta(ref mut m) => {
                    let sel = VPE::cur().alloc_sel();
                    let msize = xchg.in_args().pop_word()?;
                    log!(
                        crate::LOG_DEF,
                        "[{}] pipes::new_pipe(sid={}, sel={}, size={:#x})",
                        sid,
                        nsid,
                        sel,
                        msize
                    );
                    let pipe = m.create_pipe(nsid, msize as usize);
                    let nsess = self.new_sub_sess(crt, sel, nsid, SessionData::Pipe(pipe))?;
                    Ok((nsid, nsess, false))
                },

                // pipe sessions allow to create new channels
                SessionData::Pipe(ref mut p) => {
                    let sel = VPE::cur().alloc_sels(2);
                    let ty = match xchg.in_args().pop_word()? {
                        1 => ChanType::READ,
                        _ => ChanType::WRITE,
                    };
                    log!(
                        crate::LOG_DEF,
                        "[{}] pipes::new_chan(sid={}, sel={}, ty={:?})",
                        sid,
                        nsid,
                        sel,
                        ty
                    );
                    let chan = p.new_chan(nsid, sel, ty)?;
                    p.attach(&chan);
                    let nsess = self.new_sub_sess(crt, sel, nsid, SessionData::Chan(chan))?;
                    Ok((nsid, nsess, false))
                },

                // channel sessions can be cloned
                SessionData::Chan(ref mut c) => {
                    let sel = VPE::cur().alloc_sels(2);
                    log!(
                        crate::LOG_DEF,
                        "[{}] pipes::clone(sid={}, sel={})",
                        sid,
                        nsid,
                        sel
                    );

                    let chan = c.clone(nsid, sel)?;
                    let nsess = self.new_sub_sess(crt, sel, nsid, SessionData::Chan(chan))?;
                    Ok((nsid, nsess, true))
                },
            }
        };
        let (nsid, nsess, attach_pipe) = res?;

        let crd = if let SessionData::Chan(ref c) = nsess.data() {
            // workaround because we cannot borrow self.sessions again inside the above match
            if attach_pipe {
                let psess = self.sessions.get_mut(c.pipe_sess()).unwrap();
                if let SessionData::Pipe(ref mut p) = psess.data_mut() {
                    p.attach(c);
                }
            }

            c.crd()
        }
        else {
            kif::CapRngDesc::new(kif::CapType::OBJECT, nsess.sel(), 1)
        };

        // cannot fail because of the check above
        self.sessions.add(crt, nsid, nsess).unwrap();

        xchg.out_caps(crd);

        Ok(())
    }

    fn delegate(&mut self, _crt: usize, sid: SessId, xchg: &mut CapExchange) -> Result<(), Error> {
        let sess = self.sessions.get_mut(sid).unwrap();
        match &mut sess.data_mut() {
            // pipe sessions expect a memory cap for the shared memory of the pipe
            SessionData::Pipe(ref mut p) => {
                if xchg.in_caps() != 1 || p.has_mem() {
                    return Err(Error::new(Code::InvArgs));
                }

                let sel = VPE::cur().alloc_sel();
                log!(crate::LOG_DEF, "[{}] pipes::set_mem(sel={})", sid, sel);
                p.set_mem(sel);
                xchg.out_caps(kif::CapRngDesc::new(kif::CapType::OBJECT, sel, 1));

                Ok(())
            },

            // channel sessions expect an EP cap to get access to the data
            SessionData::Chan(ref mut c) => {
                if xchg.in_caps() != 1 {
                    return Err(Error::new(Code::InvArgs));
                }

                let sel = VPE::cur().alloc_sel();
                log!(crate::LOG_DEF, "[{}] pipes::set_ep(sel={})", sid, sel);
                c.set_ep(sel);
                xchg.out_caps(kif::CapRngDesc::new(kif::CapType::OBJECT, sel, 1));

                Ok(())
            },

            SessionData::Meta(_) => Err(Error::new(Code::InvArgs)),
        }
    }

    fn close(&mut self, _crt: usize, sid: SessId) {
        self.close_sess(sid).ok();
    }
}

#[no_mangle]
pub fn main() -> i32 {
    let mut hdl = PipesHandler {
        sel: 0,
        sessions: SessionContainer::new(DEF_MAX_CLIENTS),
    };
    let s = Server::new("pipes", &mut hdl).expect("Unable to create service 'pipes'");
    hdl.sel = s.sel();

    REQHDL.set(RequestHandler::default().expect("Unable to create request handler"));

    server_loop(|| {
        s.handle_ctrl_chan(&mut hdl)?;

        REQHDL.get_mut().handle(|op, mut is| {
            match op {
                GenFileOp::NEXT_IN => hdl.with_chan(&mut is, |c, is| c.next_in(is)),
                GenFileOp::NEXT_OUT => hdl.with_chan(&mut is, |c, is| c.next_out(is)),
                GenFileOp::COMMIT => hdl.with_chan(&mut is, |c, is| c.commit(is)),
                GenFileOp::CLOSE => {
                    let sid = is.label() as SessId;
                    // reply before we destroy the client's sgate. otherwise the client might
                    // notice the invalidated sgate before getting the reply and therefore give
                    // up before receiving the reply a bit later anyway. this in turn causes
                    // trouble if the receive gate (with the reply) is reused for something else.
                    reply_vmsg!(is, 0).ok();
                    hdl.close_sess(sid)
                },
                GenFileOp::STAT => Err(Error::new(Code::NotSup)),
                GenFileOp::SEEK => Err(Error::new(Code::NotSup)),
                _ => Err(Error::new(Code::InvArgs)),
            }
        })
    })
    .ok();

    0
}
