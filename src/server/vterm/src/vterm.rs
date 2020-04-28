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

#[macro_use]
extern crate m3;

use core::mem::MaybeUninit;
use m3::cap::Selector;
use m3::cell::LazyStaticCell;
use m3::com::{GateIStream, MemGate, Perm, SGateArgs, SendGate};
use m3::errors::{Code, Error};
use m3::io::{Read, Serial, Write};
use m3::kif;
use m3::pes::VPE;
use m3::serialize::Source;
use m3::server::{
    server_loop, CapExchange, Handler, RequestHandler, Server, SessId, SessionContainer,
    DEF_MAX_CLIENTS,
};
use m3::session::ServerSession;
use m3::syscalls;
use m3::tcu::Label;
use m3::vfs::GenFileOp;

pub const LOG_DEF: bool = false;

const BUF_SIZE: usize = 256;

static REQHDL: LazyStaticCell<RequestHandler> = LazyStaticCell::default();

#[derive(Debug)]
struct VTermSession {
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
    mem: MemGate,
    pos: usize,
    len: usize,
}

impl Channel {
    fn new(id: SessId, mem: &MemGate, caps: Selector, writing: bool) -> Result<Self, Error> {
        let sgate = SendGate::new_with(
            SGateArgs::new(REQHDL.recv_gate())
                .label(id as Label)
                .credits(1)
                .sel(caps + 1),
        )?;
        let cmem = mem.derive(id as u64 * BUF_SIZE as u64, BUF_SIZE, kif::Perm::RW)?;

        Ok(Channel {
            id,
            active: false,
            writing,
            ep: None,
            sgate,
            mem: cmem,
            pos: 0,
            len: 0,
        })
    }

    fn activate(&mut self) -> Result<(), Error> {
        if !self.active {
            let ep = self.ep.ok_or_else(|| Error::new(Code::InvArgs))?;
            syscalls::activate(ep, self.mem.sel(), kif::INVALID_SEL, 0)?;
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
            self.mem.write(&buf[0..len], 0)?;
            self.len = len;
            self.pos = 0;
        }

        self.activate()?;

        reply_vmsg!(is, 0, self.pos, self.len - self.pos)
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

        reply_vmsg!(is, 0, 0usize, BUF_SIZE)
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

        reply_vmsg!(is, 0)
    }

    fn flush(&mut self, nbytes: usize) -> Result<(), Error> {
        if nbytes > 0 {
            // safety: will be initialized by read below
            #[allow(clippy::uninit_assumed_init)]
            let mut buf: [u8; 256] = unsafe { MaybeUninit::uninit().assume_init() };
            self.mem.read(&mut buf[0..nbytes], 0)?;
            Serial::default().write(&buf[0..nbytes])?;
        }
        self.len = 0;
        Ok(())
    }
}

struct VTermHandler {
    sel: Selector,
    sessions: SessionContainer<VTermSession>,
    mem: MemGate,
}

impl VTermHandler {
    fn new_sess(sess: ServerSession) -> VTermSession {
        log!(crate::LOG_DEF, "[{}] vterm::new_meta()", sess.ident());
        VTermSession {
            sess,
            data: SessionData::Meta,
        }
    }

    fn new_chan(&self, sid: SessId, writing: bool) -> Result<VTermSession, Error> {
        log!(crate::LOG_DEF, "[{}] vterm::new_chan()", sid);
        let sels = VPE::cur().alloc_sels(2);
        Ok(VTermSession {
            sess: ServerSession::new_with_sel(self.sel, sels, sid as u64, false)?,
            data: SessionData::Chan(Channel::new(sid, &self.mem, sels, writing)?),
        })
    }

    fn close_sess(&mut self, sid: SessId) -> Result<(), Error> {
        log!(crate::LOG_DEF, "[{}] vterm::close()", sid);
        self.sessions.remove(sid);
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

impl Handler for VTermHandler {
    fn open(&mut self, srv_sel: Selector, _arg: &str) -> Result<(Selector, SessId), Error> {
        self.sessions
            .add_next(srv_sel, false, |sess| Ok(Self::new_sess(sess)))
    }

    fn obtain(&mut self, sid: SessId, xchg: &mut CapExchange) -> Result<(), Error> {
        if xchg.in_caps() != 2 {
            return Err(Error::new(Code::InvArgs));
        }

        let (nsid, nsess) = {
            let sessions = &self.sessions;
            let nsid = sessions.next_id()?;
            let sess = sessions.get(sid).unwrap();
            match &sess.data {
                SessionData::Meta => self
                    .new_chan(nsid, xchg.in_args().pop_word()? == 1)
                    .map(|s| (nsid, s)),

                SessionData::Chan(c) => self.new_chan(nsid, c.writing).map(|s| (nsid, s)),
            }
        }?;

        let sel = nsess.sess.sel();
        self.sessions.add(nsid, nsess);

        xchg.out_caps(kif::CapRngDesc::new(kif::CapType::OBJECT, sel, 2));
        Ok(())
    }

    fn delegate(&mut self, sid: SessId, xchg: &mut CapExchange) -> Result<(), Error> {
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

    fn close(&mut self, sid: SessId) {
        self.close_sess(sid).ok();
    }
}

#[no_mangle]
pub fn main() -> i32 {
    let s = Server::new("vterm").expect("Unable to create service 'vterm'");

    let mut hdl = VTermHandler {
        sel: s.sel(),
        sessions: SessionContainer::new(DEF_MAX_CLIENTS),
        mem: MemGate::new(DEF_MAX_CLIENTS * BUF_SIZE, Perm::RW).expect("Unable to alloc memory"),
    };

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
