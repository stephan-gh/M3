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
use m3::cell::RefCell;
use m3::com::*;
use m3::errors::{Code, Error};
use m3::io::{Read, Serial, Write};
use m3::kif;
use m3::server::{server_loop, Handler, Server, SessId, SessionContainer};
use m3::session::ServerSession;
use m3::syscalls;
use m3::util;
use m3::vfs::GenFileOp;
use m3::vpe::VPE;

const MSG_SIZE: usize = 64;
const BUF_SIZE: usize = 256;
const MAX_CLIENTS: usize = 32;

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
    fn new(
        id: SessId,
        rgate: &RecvGate,
        mem: &MemGate,
        caps: Selector,
        writing: bool,
    ) -> Result<Self, Error> {
        let sgate = SendGate::new_with(
            SGateArgs::new(rgate)
                .label(id as u64)
                .credits(MSG_SIZE as u64)
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
            syscalls::activate(ep, self.mem.sel(), 0)?;
            self.active = true;
        }
        Ok(())
    }

    fn next_in(&mut self, is: &mut GateIStream) -> Result<(), Error> {
        log!(VTERM, "[{}] vterm::next_in()", self.id);

        if self.writing {
            return Err(Error::new(Code::NoPerm));
        }

        self.pos += self.len - self.pos;

        if self.pos == self.len {
            let mut buf: [u8; 256] = unsafe { MaybeUninit::uninit().assume_init() };
            let len = Serial::new().borrow_mut().read(&mut buf)?;
            self.mem.write(&buf[0..len], 0)?;
            self.len = len;
            self.pos = 0;
        }

        self.activate()?;

        reply_vmsg!(is, 0, self.pos, self.len - self.pos)
    }

    fn next_out(&mut self, is: &mut GateIStream) -> Result<(), Error> {
        log!(VTERM, "[{}] vterm::next_out()", self.id);

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
        let nbytes: usize = is.pop();

        log!(VTERM, "[{}] vterm::commit(nbytes={})", self.id, nbytes);

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
            let mut buf: [u8; 256] = unsafe { MaybeUninit::uninit().assume_init() };
            self.mem.read(&mut buf[0..nbytes], 0)?;
            Serial::new().borrow_mut().write(&buf[0..nbytes])?;
        }
        self.len = 0;
        Ok(())
    }
}

struct VTermHandler {
    sel: Selector,
    sessions: RefCell<SessionContainer<VTermSession>>,
    mem: MemGate,
    rgate: RecvGate,
}

impl VTermHandler {
    fn new_sess(
        &self,
        sid: SessId,
        srv_sel: Selector,
        sel: Selector,
        data: SessionData,
    ) -> Result<VTermSession, Error> {
        let sess = ServerSession::new_with_sel(srv_sel, sel, sid as u64)?;

        Ok(VTermSession { sess, data })
    }

    fn new_chan(&self, sid: SessId, writing: bool) -> Result<VTermSession, Error> {
        let sels = VPE::cur().alloc_sels(2);
        self.new_sess(
            sid,
            self.sel,
            sels,
            SessionData::Chan(Channel::new(sid, &self.rgate, &self.mem, sels, writing)?),
        )
    }

    fn close_sess(&self, sid: SessId) -> Result<(), Error> {
        log!(VTERM, "[{}] vterm::close()", sid);
        self.sessions.borrow_mut().remove(sid);
        Ok(())
    }
}

impl Handler for VTermHandler {
    fn open(&mut self, srv_sel: Selector, _arg: &str) -> Result<(Selector, SessId), Error> {
        let sid = self.sessions.borrow().next_id()?;
        let sel = VPE::cur().alloc_sel();
        let sess = self.new_sess(sid, srv_sel, sel, SessionData::Meta)?;
        self.sessions.borrow_mut().add(sid, sess);
        log!(VTERM, "[{}] vterm::new_meta()", sid);
        Ok((sel, sid))
    }

    fn obtain(&mut self, sid: SessId, data: &mut kif::service::ExchangeData) -> Result<(), Error> {
        if data.caps != 2 {
            return Err(Error::new(Code::InvArgs));
        }

        let (nsid, nsess) = {
            let sessions = self.sessions.borrow();
            let nsid = sessions.next_id()?;
            let sess = sessions.get(sid).unwrap();
            match &sess.data {
                SessionData::Meta => {
                    if data.args.count != 1 {
                        return Err(Error::new(Code::InvArgs));
                    }
                    self.new_chan(nsid, data.args.ival(0) == 1)
                        .map(|s| (nsid, s))
                },

                SessionData::Chan(c) => {
                    if data.args.count != 0 {
                        return Err(Error::new(Code::InvArgs));
                    }
                    self.new_chan(nsid, c.writing).map(|s| (nsid, s))
                },
            }
        }?;

        log!(VTERM, "[{}] vterm::new_chan()", nsid);

        let sel = nsess.sess.sel();
        self.sessions.borrow_mut().add(nsid, nsess);

        data.caps = kif::CapRngDesc::new(kif::CapType::OBJECT, sel, 2).value();
        Ok(())
    }

    fn delegate(
        &mut self,
        sid: SessId,
        data: &mut kif::service::ExchangeData,
    ) -> Result<(), Error> {
        if data.caps != 1 || data.args.count != 0 {
            return Err(Error::new(Code::InvArgs));
        }

        let mut sessions = self.sessions.borrow_mut();
        let sess = sessions.get_mut(sid).unwrap();
        match &mut sess.data {
            SessionData::Meta => Err(Error::new(Code::InvArgs)),
            SessionData::Chan(c) => {
                let sel = VPE::cur().alloc_sel();
                c.ep = Some(sel);
                data.caps = kif::CapRngDesc::new(kif::CapType::OBJECT, sel, 1).value();
                Ok(())
            },
        }
    }

    fn close(&mut self, sid: SessId) {
        self.close_sess(sid).ok();
    }
}

impl VTermHandler {
    pub fn new(sel: Selector) -> Result<Self, Error> {
        let mut rgate = RecvGate::new(
            util::next_log2(MAX_CLIENTS * MSG_SIZE),
            util::next_log2(MSG_SIZE),
        )?;
        rgate.activate()?;
        Ok(VTermHandler {
            sel,
            sessions: RefCell::new(SessionContainer::new(MAX_CLIENTS)),
            mem: MemGate::new(MAX_CLIENTS * BUF_SIZE, Perm::RW)?,
            rgate,
        })
    }

    pub fn handle(&mut self) -> Result<(), Error> {
        if let Some(mut is) = self.rgate.fetch() {
            let res = match is.pop() {
                GenFileOp::NEXT_IN => self.with_chan(&mut is, |c, is| c.next_in(is)),
                GenFileOp::NEXT_OUT => self.with_chan(&mut is, |c, is| c.next_out(is)),
                GenFileOp::COMMIT => self.with_chan(&mut is, |c, is| c.commit(is)),
                GenFileOp::CLOSE => {
                    let sid = is.label() as SessId;
                    // reply before we destroy the client's sgate. otherwise the client might
                    // notice the invalidated sgate before getting the reply and therefore give
                    // up before receiving the reply a bit later anyway. this in turn causes
                    // trouble if the receive gate (with the reply) is reused for something else.
                    reply_vmsg!(is, 0).ok();
                    self.close_sess(sid)
                },
                GenFileOp::STAT => Err(Error::new(Code::NotSup)),
                GenFileOp::SEEK => Err(Error::new(Code::NotSup)),
                _ => Err(Error::new(Code::InvArgs)),
            };

            if let Err(e) = res {
                is.reply_error(e.code()).ok();
            }
        }

        Ok(())
    }

    fn with_chan<F, R>(&self, is: &mut GateIStream, func: F) -> Result<R, Error>
    where
        F: Fn(&mut Channel, &mut GateIStream) -> Result<R, Error>,
    {
        let mut sessions = self.sessions.borrow_mut();
        let sess = sessions.get_mut(is.label() as SessId).unwrap();
        match &mut sess.data {
            SessionData::Meta => Err(Error::new(Code::InvArgs)),
            SessionData::Chan(c) => func(c, is),
        }
    }
}

#[no_mangle]
pub fn main() -> i32 {
    let s = Server::new("vterm").expect("Unable to create service 'vterm'");

    let mut hdl = VTermHandler::new(s.sel()).expect("Unable to create handler");

    server_loop(|| {
        s.handle_ctrl_chan(&mut hdl)?;

        hdl.handle()
    })
    .ok();

    0
}
