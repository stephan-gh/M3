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

use m3::cap::Selector;
use m3::cell::RefCell;
use m3::col::{VarRingBuf, Vec};
use m3::com::{GateIStream, MemGate, SGateArgs, SendGate};
use m3::errors::{Code, Error};
use m3::kif;
use m3::rc::Rc;
use m3::server::SessId;
use m3::session::ServerSession;
use m3::syscalls;
use m3::tcu::{Label, Message};

use rgate;

macro_rules! reply_vmsg_late {
    ( $rgate:expr, $msg:expr, $( $args:expr ),* ) => ({
        let mut os = m3::com::GateOStream::default();
        $( os.push(&$args); )*
        $rgate.reply(os.words(), $msg)
    });
}

pub struct PipesSession {
    sess: ServerSession,
    data: SessionData,
}

impl PipesSession {
    pub fn new(sess: ServerSession, data: SessionData) -> Self {
        PipesSession { sess, data }
    }

    pub fn sel(&self) -> Selector {
        self.sess.sel()
    }

    pub fn data(&self) -> &SessionData {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut SessionData {
        &mut self.data
    }
}

pub enum SessionData {
    Meta(Meta),
    Pipe(Pipe),
    Chan(Channel),
}

#[derive(Default)]
pub struct Meta {
    pipes: Vec<SessId>,
}

impl Meta {
    pub fn create_pipe(&mut self, sid: SessId, mem_size: usize) -> Pipe {
        self.pipes.push(sid);
        Pipe::new(sid, mem_size)
    }

    pub fn close(&mut self, sids: &mut Vec<SessId>) -> Result<(), Error> {
        sids.extend_from_slice(&self.pipes);
        Ok(())
    }
}

bitflags! {
    struct Flags : u64 {
        const WRITE_EOF = 0x1;
        const READ_EOF  = 0x2;
    }
}

struct PendingRequest {
    chan: SessId,
    msg: &'static Message,
}

impl PendingRequest {
    fn new(chan: SessId, msg: &'static Message) -> Self {
        PendingRequest { chan, msg }
    }
}

struct State {
    flags: Flags,
    mem: Option<MemGate>,
    mem_size: usize,
    rbuf: VarRingBuf,
    last_read: Option<(SessId, usize)>,
    last_write: Option<(SessId, usize)>,
    pending_reads: Vec<PendingRequest>,
    pending_writes: Vec<PendingRequest>,
    reader: Vec<SessId>,
    writer: Vec<SessId>,
}

impl State {
    fn new(mem_size: usize) -> Self {
        State {
            flags: Flags::empty(),
            mem: None,
            mem_size,
            rbuf: VarRingBuf::new(mem_size),
            last_read: None,
            last_write: None,
            pending_reads: Vec::new(),
            pending_writes: Vec::new(),
            reader: Vec::new(),
            writer: Vec::new(),
        }
    }

    fn get_read_size(&self) -> usize {
        assert!(self.reader.len() > 0);
        self.rbuf.size() / (4 * self.reader.len())
    }

    fn get_write_size(&self) -> usize {
        assert!(self.writer.len() > 0);
        self.rbuf.size() / (4 * self.writer.len())
    }

    fn append_request(&mut self, id: SessId, is: &mut GateIStream, read: bool) {
        let req = PendingRequest::new(id, is.take_msg());
        if read {
            log!(crate::LOG_DEF, "[{}] pipes::read_wait()", id);
            self.pending_reads.insert(0, req);
        }
        else {
            log!(crate::LOG_DEF, "[{}] pipes::write_wait()", id);
            self.pending_writes.insert(0, req);
        }
    }

    fn handle_pending_reads(&mut self) {
        // if a read is still in progress, we cannot start other reads
        if self.last_read.is_some() {
            return;
        }

        // use a loop here, because if we are at write-EOF, we want to report EOF to all readers
        while let Some(req) = self.pending_reads.last() {
            // try to find a place to read from
            let amount = self.get_read_size();
            if let Some((pos, amount)) = self.rbuf.get_read_pos(amount) {
                // start reading
                self.last_read = Some((req.chan, amount));
                log!(
                    crate::LOG_DEF,
                    "[{}] pipes::late_read(): {} @ {}",
                    req.chan,
                    amount,
                    pos
                );
                reply_vmsg_late!(rgate(), req.msg, 0, pos, amount).ok();

                // remove write request
                self.pending_reads.pop();
                break;
            }
            // did all writers leave?
            else if self.flags.contains(Flags::WRITE_EOF) {
                // report EOF
                log!(crate::LOG_DEF, "[{}] pipes::late_read(): EOF", req.chan);
                reply_vmsg_late!(rgate(), req.msg, 0, 0usize, 0usize).ok();

                // remove write request
                self.pending_reads.pop();
            }
            else {
                // otherwise, don't consider more read requests
                break;
            }
        }
    }

    fn handle_pending_writes(&mut self) {
        // if a write is still in progress, we cannot start other writes
        if self.last_write.is_some() {
            return;
        }

        // if all readers left, just report EOF to all pending write requests
        if self.flags.contains(Flags::READ_EOF) {
            while let Some(req) = self.pending_writes.pop() {
                log!(crate::LOG_DEF, "[{}] pipes::late_write(): EOF", req.chan);
                reply_vmsg_late!(rgate(), req.msg, Code::EndOfFile as u32).ok();
            }
        }
        // is there a pending write request?
        else if let Some(req) = self.pending_writes.last() {
            // try to find a place to write
            let amount = self.get_write_size();
            if let Some(pos) = self.rbuf.get_write_pos(amount) {
                // start writing
                self.last_write = Some((req.chan, amount));
                log!(
                    crate::LOG_DEF,
                    "[{}] pipes::late_write(): {} @ {}",
                    req.chan,
                    amount,
                    pos
                );
                reply_vmsg_late!(rgate(), req.msg, 0, pos, amount).ok();

                // remove write request
                self.pending_writes.pop();
            }
        }
    }

    fn remove_pending(&mut self, read: bool, chan: SessId) {
        let list = if read {
            &mut self.pending_reads
        }
        else {
            &mut self.pending_writes
        };
        list.retain(|req| req.chan != chan);
    }
}

pub struct Pipe {
    id: SessId,
    state: Rc<RefCell<State>>,
}

impl Pipe {
    pub fn new(id: SessId, mem_size: usize) -> Self {
        Pipe {
            id,
            state: Rc::new(RefCell::new(State::new(mem_size))),
        }
    }

    pub fn has_mem(&self) -> bool {
        self.state.borrow().mem.is_some()
    }

    pub fn set_mem(&mut self, sel: Selector) {
        self.state.borrow_mut().mem = Some(MemGate::new_bind(sel));
    }

    pub fn new_chan(&self, sid: SessId, sel: Selector, ty: ChanType) -> Result<Channel, Error> {
        Channel::new(sid, sel, ty, self.id, self.state.clone())
    }

    pub fn attach(&mut self, chan: &Channel) {
        assert!(chan.pipe == self.id);
        match chan.i.ty {
            ChanType::READ => self.state.borrow_mut().reader.push(chan.id),
            ChanType::WRITE => self.state.borrow_mut().writer.push(chan.id),
        }
    }

    pub fn close(&mut self, sids: &mut Vec<SessId>) -> Result<(), Error> {
        let state = self.state.borrow();
        sids.extend_from_slice(&state.reader);
        sids.extend_from_slice(&state.writer);
        Ok(())
    }
}

#[derive(Copy, Clone, Debug)]
pub enum ChanType {
    READ,
    WRITE,
}

// TODO this struct is packed atm to avoid a code generation bug on ARM (as far as I can see)
#[repr(C, packed)]
struct ChanIntern {
    ty: ChanType,
    mem: Option<MemGate>,
}

pub struct Channel {
    i: ChanIntern,
    id: SessId,
    pipe: SessId,
    state: Rc<RefCell<State>>,
    sgate: SendGate,
    ep_cap: Option<Selector>,
}

impl Channel {
    fn new(
        id: SessId,
        sel: Selector,
        ty: ChanType,
        pipe: SessId,
        state: Rc<RefCell<State>>,
    ) -> Result<Self, Error> {
        let sgate = SendGate::new_with(
            SGateArgs::new(rgate())
                .label(id as Label)
                .credits(1)
                .sel(sel + 1),
        )?;
        Ok(Channel {
            i: ChanIntern { ty, mem: None },
            id,
            pipe,
            state,
            sgate,
            ep_cap: None,
        })
    }

    pub fn pipe_sess(&self) -> SessId {
        self.pipe
    }

    pub fn crd(&self) -> kif::CapRngDesc {
        kif::CapRngDesc::new(kif::CapType::OBJECT, self.sgate.sel() - 1, 2)
    }

    pub fn clone(&self, id: SessId, sel: Selector) -> Result<Channel, Error> {
        Channel::new(id, sel, self.i.ty, self.pipe, self.state.clone())
    }

    pub fn set_ep(&mut self, ep: Selector) {
        self.ep_cap = Some(ep);
    }

    pub fn next_in(&mut self, is: &mut GateIStream) -> Result<(), Error> {
        log!(crate::LOG_DEF, "[{}] pipes::next_in()", self.id);

        let res = match self.i.ty {
            ChanType::READ => self.read(is, 0),
            ChanType::WRITE => Err(Error::new(Code::InvArgs)),
        };

        self.state.borrow_mut().handle_pending_writes();
        res
    }

    pub fn next_out(&mut self, is: &mut GateIStream) -> Result<(), Error> {
        log!(crate::LOG_DEF, "[{}] pipes::next_out()", self.id);

        let res = match self.i.ty {
            ChanType::READ => Err(Error::new(Code::InvArgs)),
            ChanType::WRITE => self.write(is, 0),
        };

        self.state.borrow_mut().handle_pending_reads();
        res
    }

    pub fn commit(&mut self, is: &mut GateIStream) -> Result<(), Error> {
        let nbytes: usize = is.pop()?;

        log!(
            crate::LOG_DEF,
            "[{}] pipes::commit(nbytes={})",
            self.id,
            nbytes
        );

        let res = match self.i.ty {
            ChanType::READ => self.read(is, nbytes),
            ChanType::WRITE => self.write(is, nbytes),
        };

        self.handle_pending();
        res
    }

    pub fn close(&mut self, _sids: &mut Vec<SessId>) -> Result<(), Error> {
        let res = match self.i.ty {
            ChanType::READ => self.close_reader(),
            ChanType::WRITE => self.close_writer(),
        };

        self.handle_pending();
        res
    }

    fn handle_pending(&mut self) {
        match self.i.ty {
            ChanType::READ => self.state.borrow_mut().handle_pending_writes(),
            ChanType::WRITE => self.state.borrow_mut().handle_pending_reads(),
        }
    }

    fn read(&mut self, is: &mut GateIStream, commit: usize) -> Result<(), Error> {
        self.activate()?;

        // if a read is in progress, we have to commit it
        let mut state = self.state.borrow_mut();
        if let Some((last_id, last_amount)) = state.last_read {
            // if that wasn't the same client, queue the read request
            if last_id != self.id {
                // commits cannot be queued
                if commit > 0 {
                    return Err(Error::new(Code::InvArgs));
                }
                state.append_request(self.id, is, true);
                return Ok(());
            }

            // this client is the current reader, so commit the read by pulling it from the ringbuf
            let amount = if commit == 0 { last_amount } else { commit };
            log!(crate::LOG_DEF, "[{}] pipes::read_pull({})", self.id, amount);
            state.rbuf.pull(amount);
            state.last_read = None;
        }

        // commits are done here, because they don't get new data
        if commit > 0 {
            return reply_vmsg!(is, 0, state.rbuf.size());
        }

        // if there are already queued read requests, just append this request
        if state.pending_reads.len() > 0 {
            // only queue the request if we still have writers
            if !state.flags.contains(Flags::WRITE_EOF) {
                state.append_request(self.id, is, true);
                return Ok(());
            }
        }

        // request new read position
        let amount = state.get_read_size();
        if let Some((pos, amount)) = state.rbuf.get_read_pos(amount) {
            // there is something to read; give client the position and size
            state.last_read = Some((self.id, amount));
            log!(
                crate::LOG_DEF,
                "[{}] pipes::read(): {} @ {}",
                self.id,
                amount,
                pos
            );
            reply_vmsg!(is, 0, pos, amount)
        }
        else {
            // nothing to read; if there is no writer left, report EOF
            if state.flags.contains(Flags::WRITE_EOF) {
                log!(crate::LOG_DEF, "[{}] pipes::read(): EOF", self.id);
                reply_vmsg!(is, 0, 0usize, 0usize)
            }
            // otherwise queue the request
            else {
                state.append_request(self.id, is, true);
                Ok(())
            }
        }
    }

    fn write(&mut self, is: &mut GateIStream, commit: usize) -> Result<(), Error> {
        self.activate()?;

        // if there are no readers left, report EOF
        let mut state = self.state.borrow_mut();
        if state.flags.contains(Flags::READ_EOF) {
            log!(crate::LOG_DEF, "[{}] pipes::write(): EOF", self.id);
            return is.reply_error(Code::EndOfFile);
        }

        // is a write in progress?
        if let Some((last_id, last_amount)) = state.last_write {
            // if that wasn't the same client, queue the write request
            if last_id != self.id {
                // commits cannot be queued
                if commit > 0 {
                    return Err(Error::new(Code::InvArgs));
                }
                state.append_request(self.id, is, false);
                return Ok(());
            }

            // this client is the current reader, so commit the write by pushing it to the ringbuf
            let amount = if commit == 0 { last_amount } else { commit };
            log!(
                crate::LOG_DEF,
                "[{}] pipes::write_push({})",
                self.id,
                amount
            );
            state.rbuf.push(last_amount, amount);
            state.last_write = None;
        }

        // commits are done here, because they don't get new data
        if commit > 0 {
            return reply_vmsg!(is, 0, state.rbuf.size());
        }

        // if there are already queued write requests, just append this request
        if state.pending_writes.len() > 0 {
            state.append_request(self.id, is, false);
            return Ok(());
        }

        // request new write position
        let amount = state.get_write_size();
        if let Some(pos) = state.rbuf.get_write_pos(amount) {
            // there is space to write; give client the position and size
            state.last_write = Some((self.id, amount));
            log!(
                crate::LOG_DEF,
                "[{}] pipes::write(): {} @ {}",
                self.id,
                amount,
                pos
            );
            reply_vmsg!(is, 0, pos, amount)
        }
        else {
            // nothing to write, so queue the request
            state.append_request(self.id, is, false);
            Ok(())
        }
    }

    fn close_reader(&mut self) -> Result<(), Error> {
        let mut state = self.state.borrow_mut();
        state.remove_pending(true, self.id);

        // if we're already at read-EOF, there is something wrong
        if state.flags.contains(Flags::READ_EOF) {
            return Err(Error::new(Code::InvArgs));
        }

        // is a read in progress?
        if let Some((last_id, _)) = state.last_read {
            // pull it from the ring buffer, if it's this client's read
            if last_id == self.id {
                log!(crate::LOG_DEF, "[{}] pipes::read_pull(): 0", self.id);
                state.rbuf.pull(0);
                state.last_read = None;
            }
            // otherwise, we ignore it because the client violated the protocol
        }

        // remove client
        state.reader.remove_item(&(self.id as usize));
        let rd_left = state.reader.len();
        if rd_left > 0 {
            log!(
                crate::LOG_DEF,
                "[{}] pipes::close(): rd-refs={}",
                self.id,
                rd_left
            );
            return Ok(());
        }

        // no readers left: EOF
        state.flags.insert(Flags::READ_EOF);
        log!(crate::LOG_DEF, "[{}] pipes::close(): read EOF", self.id);
        Ok(())
    }

    fn close_writer(&mut self) -> Result<(), Error> {
        let mut state = self.state.borrow_mut();
        state.remove_pending(false, self.id);

        // if we're already at write-EOF, there is something wrong
        if state.flags.contains(Flags::WRITE_EOF) {
            return Err(Error::new(Code::InvArgs));
        }

        // is a write in progress?
        if let Some((last_id, last_amount)) = state.last_write {
            // push it to the ring buffer, if it's this client's read
            if last_id == self.id {
                log!(crate::LOG_DEF, "[{}] pipes::write_push(): 0", self.id);
                state.rbuf.push(last_amount, 0);
                state.last_write = None;
            }
            // otherwise, we ignore it because the client violated the protocol
        }

        // remove client
        state.writer.remove_item(&(self.id as usize));
        let wr_left = state.writer.len();
        if wr_left > 0 {
            log!(
                crate::LOG_DEF,
                "[{}] pipes::close(): wr-refs={}",
                self.id,
                wr_left
            );
            return Ok(());
        }

        // no writers left: EOF
        state.flags.insert(Flags::WRITE_EOF);
        log!(crate::LOG_DEF, "[{}] pipes::close(): write EOF", self.id);
        Ok(())
    }

    fn activate(&mut self) -> Result<(), Error> {
        // did we get an EP cap from the client?
        if let Some(cap) = self.ep_cap.take() {
            unsafe {
                assert!(self.i.mem.is_none());
            }

            // did we get a memory cap from the client?
            let state = self.state.borrow();
            if let Some(mem) = &state.mem {
                // derive read-only/write-only mem cap
                let perm = match self.i.ty {
                    ChanType::READ => kif::Perm::R,
                    ChanType::WRITE => kif::Perm::W,
                };
                let cmem = mem.derive(0, state.mem_size, perm)?;
                // activate it on client's EP
                log!(
                    crate::LOG_DEF,
                    "[{}] pipes::activate(ep={}, gate={})",
                    self.id,
                    cap,
                    cmem.sel()
                );
                syscalls::activate(cap, cmem.sel(), 0)?;
                self.i.mem = Some(cmem);
            }
            else {
                return Err(Error::new(Code::InvArgs));
            }
        }
        Ok(())
    }
}
