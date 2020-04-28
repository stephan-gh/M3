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
use m3::cell::StaticCell;
use m3::com::{GateIStream, RecvGate};
use m3::errors::{Code, Error};
use m3::kif;
use m3::math;
use m3::pes::VPE;
use m3::serialize::Source;
use m3::server::{server_loop, CapExchange, Handler, Server, SessId, SessionContainer};
use m3::session::ServerSession;
use m3::tcu::Label;
use m3::vfs::GenFileOp;

use sess::{ChanType, Channel, Meta, PipesSession, SessionData};

pub const LOG_DEF: bool = false;

const MSG_SIZE: usize = 64;
const MAX_CLIENTS: usize = 32;

static RGATE: StaticCell<Option<RecvGate>> = StaticCell::new(None);

fn rgate() -> &'static RecvGate {
    RGATE.get().as_ref().unwrap()
}

struct PipesHandler {
    sel: Selector,
    sessions: SessionContainer<PipesSession>,
}

impl PipesHandler {
    fn new_sub_sess(
        &self,
        sel: Selector,
        nsid: SessId,
        data: SessionData,
    ) -> Result<PipesSession, Error> {
        // let the kernel close the session as soon as the client dies or the session is revoked
        // for some other reason. this is required to signal EOF to the other side of the pipe.
        Ok(PipesSession::new(
            ServerSession::new_with_sel(self.sel, sel, nsid as u64, true)?,
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

                self.sessions.remove(id);
                // ignore all potentially outstanding messages of this session
                rgate().drop_msgs_with(id as Label);
            }
        }
        Ok(())
    }
}

impl Handler for PipesHandler {
    fn open(&mut self, srv_sel: Selector, _arg: &str) -> Result<(Selector, SessId), Error> {
        self.sessions.add_next(srv_sel, true, |sess| {
            log!(crate::LOG_DEF, "[{}] pipes::new_meta()", sess.ident());
            Ok(PipesSession::new(sess, SessionData::Meta(Meta::default())))
        })
    }

    fn obtain(&mut self, sid: SessId, xchg: &mut CapExchange) -> Result<(), Error> {
        if xchg.in_caps() != 2 {
            return Err(Error::new(Code::InvArgs));
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
                    let nsess = self.new_sub_sess(sel, nsid, SessionData::Pipe(pipe))?;
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
                    let nsess = self.new_sub_sess(sel, nsid, SessionData::Chan(chan))?;
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
                    let nsess = self.new_sub_sess(sel, nsid, SessionData::Chan(chan))?;
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

        self.sessions.add(nsid, nsess);

        xchg.out_caps(crd);

        Ok(())
    }

    fn delegate(&mut self, sid: SessId, xchg: &mut CapExchange) -> Result<(), Error> {
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

    fn close(&mut self, sid: SessId) {
        self.close_sess(sid).ok();
    }
}

impl PipesHandler {
    pub fn new(sel: Selector) -> Result<Self, Error> {
        Ok(PipesHandler {
            sel,
            sessions: SessionContainer::new(MAX_CLIENTS),
        })
    }

    pub fn handle(&mut self, mut is: &mut GateIStream) -> Result<(), Error> {
        let res = match is.pop() {
            Ok(GenFileOp::NEXT_IN) => {
                Self::with_chan(&mut self.sessions, &mut is, |c, is| c.next_in(is))
            },
            Ok(GenFileOp::NEXT_OUT) => {
                Self::with_chan(&mut self.sessions, &mut is, |c, is| c.next_out(is))
            },
            Ok(GenFileOp::COMMIT) => {
                Self::with_chan(&mut self.sessions, &mut is, |c, is| c.commit(is))
            },
            Ok(GenFileOp::CLOSE) => {
                let sid = is.label() as SessId;
                // reply before we destroy the client's sgate. otherwise the client might
                // notice the invalidated sgate before getting the reply and therefore give
                // up before receiving the reply a bit later anyway. this in turn causes
                // trouble if the receive gate (with the reply) is reused for something else.
                reply_vmsg!(is, 0).ok();
                self.close_sess(sid)
            },
            Ok(GenFileOp::STAT) => Err(Error::new(Code::NotSup)),
            Ok(GenFileOp::SEEK) => Err(Error::new(Code::NotSup)),
            _ => Err(Error::new(Code::InvArgs)),
        };

        if let Err(e) = res {
            is.reply_error(e.code()).ok();
        }

        Ok(())
    }

    fn with_chan<F, R>(
        sessions: &mut SessionContainer<PipesSession>,
        is: &mut GateIStream,
        func: F,
    ) -> Result<R, Error>
    where
        F: Fn(&mut Channel, &mut GateIStream) -> Result<R, Error>,
    {
        let sess = sessions.get_mut(is.label() as SessId).unwrap();
        match &mut sess.data_mut() {
            SessionData::Chan(c) => func(c, is),
            _ => Err(Error::new(Code::InvArgs)),
        }
    }
}

#[no_mangle]
pub fn main() -> i32 {
    let s = Server::new("pipes").expect("Unable to create service 'pipes'");
    let mut hdl = PipesHandler::new(s.sel()).expect("Unable to create handler");

    let mut rg = RecvGate::new(
        math::next_log2(MAX_CLIENTS * MSG_SIZE),
        math::next_log2(MSG_SIZE),
    )
    .expect("Unable to create rgate");
    rg.activate().expect("Unable to activate rgate");
    RGATE.set(Some(rg));

    server_loop(|| {
        s.handle_ctrl_chan(&mut hdl)?;

        if let Some(mut is) = rgate().fetch() {
            hdl.handle(&mut is)
        }
        else {
            Ok(())
        }
    })
    .ok();

    0
}
