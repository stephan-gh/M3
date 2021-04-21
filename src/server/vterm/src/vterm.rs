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

use m3::cap::Selector;
use m3::cell::LazyStaticCell;
use m3::com::{GateIStream, MemGate, Perm, SGateArgs, SendGate, EP};
use m3::errors::{Code, Error};
use m3::goff;
use m3::io::{Read, Serial, Write};
use m3::kif;
use m3::log;
use m3::mem::MaybeUninit;
use m3::pes::VPE;
use m3::rc::Rc;
use m3::reply_vmsg;
use m3::server::{
    server_loop, CapExchange, Handler, RequestHandler, Server, SessId, SessionContainer,
    DEF_MAX_CLIENTS,
};
use m3::session::ServerSession;
use m3::tcu::Label;
use m3::vfs::GenFileOp;

pub const LOG_DEF: bool = false;

const BUF_SIZE: usize = 256;

static REQHDL: LazyStaticCell<RequestHandler> = LazyStaticCell::default();

#[derive(Debug)]
struct VTermSession {
    crt: usize,
    sess: ServerSession,
    data: SessionData,
}

#[derive(Debug)]
enum SessionData {
    Meta,
    Chan(Channel),
}

#[derive(Debug)]
struct Channel {
    id: SessId,
    active: bool,
    writing: bool,
    ep: Option<Selector>,
    sgate: SendGate,
    our_mem: Rc<MemGate>,
    mem: MemGate,
    pos: usize,
    len: usize,
}

fn mem_off(id: SessId) -> goff {
    id as goff * BUF_SIZE as goff
}

impl Channel {
    fn new(id: SessId, mem: Rc<MemGate>, caps: Selector, writing: bool) -> Result<Self, Error> {
        let sgate = SendGate::new_with(
            SGateArgs::new(REQHDL.recv_gate())
                .label(id as Label)
                .credits(1)
                .sel(caps + 1),
        )?;
        let cmem = mem.derive(mem_off(id), BUF_SIZE, kif::Perm::RW)?;

        Ok(Channel {
            id,
            active: false,
            writing,
            ep: None,
            sgate,
            our_mem: mem,
            mem: cmem,
            pos: 0,
            len: 0,
        })
    }

    fn activate(&mut self) -> Result<(), Error> {
        if !self.active {
            let sel = self.ep.ok_or_else(|| Error::new(Code::InvArgs))?;
            EP::new_bind(0, sel).configure(self.mem.sel())?;
            self.active = true;
        }
        Ok(())
    }

    fn next_in(&mut self, is: &mut GateIStream) -> Result<(), Error> {
        log!(crate::LOG_DEF, "[{}] vterm::next_in()", self.id);

        if self.writing {
            return Err(Error::new(Code::NoPerm));
        }

        self.pos += self.len - self.pos;

        if self.pos == self.len {
            // safety: will be initialized by read below
            #[allow(clippy::uninit_assumed_init)]
            let mut buf: [u8; 256] = unsafe { MaybeUninit::uninit().assume_init() };
            let len = Serial::default().read(&mut buf)?;
            self.our_mem.write(&buf[0..len], mem_off(self.id))?;
            self.len = len;
            self.pos = 0;
        }

        self.activate()?;

        reply_vmsg!(is, Code::None as u32, self.pos, self.len - self.pos)
    }

    fn next_out(&mut self, is: &mut GateIStream) -> Result<(), Error> {
        log!(crate::LOG_DEF, "[{}] vterm::next_out()", self.id);

        if !self.writing {
            return Err(Error::new(Code::NoPerm));
        }

        self.flush(self.len)?;
        self.activate()?;

        self.pos = 0;
        self.len = BUF_SIZE;

        reply_vmsg!(is, Code::None as u32, 0usize, BUF_SIZE)
    }

    fn commit(&mut self, is: &mut GateIStream) -> Result<(), Error> {
        let nbytes: usize = is.pop()?;

        log!(
            crate::LOG_DEF,
            "[{}] vterm::commit(nbytes={})",
            self.id,
            nbytes
        );

        if nbytes > self.len - self.pos {
            return Err(Error::new(Code::InvArgs));
        }

        if self.writing {
            self.flush(nbytes)?;
        }
        else {
            self.pos += nbytes;
        }

        is.reply_error(Code::None)
    }

    fn flush(&mut self, nbytes: usize) -> Result<(), Error> {
        if nbytes > 0 {
            // safety: will be initialized by read below
            #[allow(clippy::uninit_assumed_init)]
            let mut buf: [u8; 256] = unsafe { MaybeUninit::uninit().assume_init() };
            self.our_mem.read(&mut buf[0..nbytes], mem_off(self.id))?;
            Serial::default().write(&buf[0..nbytes])?;
        }
        self.len = 0;
        Ok(())
    }
}

struct VTermHandler {
    sel: Selector,
    sessions: SessionContainer<VTermSession>,
    mem: Rc<MemGate>,
}

impl VTermHandler {
    fn new_sess(crt: usize, sess: ServerSession) -> VTermSession {
        log!(crate::LOG_DEF, "[{}] vterm::new_meta()", sess.ident());
        VTermSession {
            crt,
            sess,
            data: SessionData::Meta,
        }
    }

    fn new_chan(&self, crt: usize, sid: SessId, writing: bool) -> Result<VTermSession, Error> {
        log!(crate::LOG_DEF, "[{}] vterm::new_chan()", sid);
        let sels = VPE::cur().alloc_sels(2);
        Ok(VTermSession {
            crt,
            sess: ServerSession::new_with_sel(self.sel, sels, crt, sid as u64, false)?,
            data: SessionData::Chan(Channel::new(sid, self.mem.clone(), sels, writing)?),
        })
    }

    fn close_sess(&mut self, sid: SessId) -> Result<(), Error> {
        log!(crate::LOG_DEF, "[{}] vterm::close()", sid);
        let crt = self.sessions.get(sid).unwrap().crt;
        self.sessions.remove(crt, sid);
        Ok(())
    }

    fn with_chan<F, R>(&mut self, is: &mut GateIStream, func: F) -> Result<R, Error>
    where
        F: Fn(&mut Channel, &mut GateIStream) -> Result<R, Error>,
    {
        let sess = self.sessions.get_mut(is.label() as SessId).unwrap();
        match &mut sess.data {
            SessionData::Meta => Err(Error::new(Code::InvArgs)),
            SessionData::Chan(c) => func(c, is),
        }
    }
}

impl Handler<VTermSession> for VTermHandler {
    fn sessions(&mut self) -> &mut m3::server::SessionContainer<VTermSession> {
        &mut self.sessions
    }

    fn open(
        &mut self,
        crt: usize,
        srv_sel: Selector,
        _arg: &str,
    ) -> Result<(Selector, SessId), Error> {
        self.sessions
            .add_next(crt, srv_sel, false, |sess| Ok(Self::new_sess(crt, sess)))
    }

    fn obtain(&mut self, crt: usize, sid: SessId, xchg: &mut CapExchange) -> Result<(), Error> {
        if xchg.in_caps() != 2 {
            return Err(Error::new(Code::InvArgs));
        }

        let (nsid, nsess) = {
            let sessions = &self.sessions;
            let nsid = sessions.next_id()?;
            let sess = sessions.get(sid).unwrap();
            match &sess.data {
                SessionData::Meta => self
                    .new_chan(crt, nsid, xchg.in_args().pop_word()? == 1)
                    .map(|s| (nsid, s)),

                SessionData::Chan(c) => self.new_chan(crt, nsid, c.writing).map(|s| (nsid, s)),
            }
        }?;

        let sel = nsess.sess.sel();
        self.sessions.add(crt, nsid, nsess)?;

        xchg.out_caps(kif::CapRngDesc::new(kif::CapType::OBJECT, sel, 2));
        Ok(())
    }

    fn delegate(&mut self, _crt: usize, sid: SessId, xchg: &mut CapExchange) -> Result<(), Error> {
        if xchg.in_caps() != 1 {
            return Err(Error::new(Code::InvArgs));
        }

        let sessions = &mut self.sessions;
        let sess = sessions.get_mut(sid).unwrap();
        match &mut sess.data {
            SessionData::Meta => Err(Error::new(Code::InvArgs)),
            SessionData::Chan(c) => {
                let sel = VPE::cur().alloc_sel();
                c.ep = Some(sel);
                xchg.out_caps(kif::CapRngDesc::new(kif::CapType::OBJECT, sel, 1));
                Ok(())
            },
        }
    }

    fn close(&mut self, _crt: usize, sid: SessId) {
        self.close_sess(sid).ok();
    }
}

#[no_mangle]
pub fn main() -> i32 {
    let mut hdl = VTermHandler {
        sel: 0,
        sessions: SessionContainer::new(DEF_MAX_CLIENTS),
        mem: Rc::new(
            MemGate::new(DEF_MAX_CLIENTS * BUF_SIZE, Perm::RW).expect("Unable to alloc memory"),
        ),
    };

    let s = Server::new("vterm", &mut hdl).expect("Unable to create service 'vterm'");
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
                    is.reply_error(Code::None).ok();
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
