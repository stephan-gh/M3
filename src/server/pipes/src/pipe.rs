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

use bitflags::bitflags;
use m3::cap::Selector;
use m3::cell::{Cell, RefCell};
use m3::col::{VarRingBuf, Vec};
use m3::com::{GateIStream, LazyGate, MemCap, RGateArgs, RecvGate, SendCap, EP};
use m3::errors::{Code, Error};
use m3::io::LogFlags;
use m3::kif;
use m3::log;
use m3::rc::Rc;
use m3::send_vmsg;
use m3::server::SessId;
use m3::tcu::Message;
use m3::vfs::FileEvent;

use crate::chan::{ChanType, Channel};

macro_rules! reply_vmsg_late {
    ( $rgate:expr, $msg:expr, $( $args:expr ),* ) => ({
        let mut msg = m3::mem::MsgBuf::borrow_def();
        m3::build_vmsg!(&mut msg, $( $args ),*);
        $rgate.reply(&msg, $msg)
    });
}

pub struct NotifyGate {
    sess: SessId,
    rgate: RecvGate,
    sgate: LazyGate<SendCap>,
    notify_events: FileEvent,
    pending_events: FileEvent,
    promised_events: Rc<Cell<FileEvent>>,
}

impl NotifyGate {
    fn new(
        sess: SessId,
        rgate: RecvGate,
        sgate: LazyGate<SendCap>,
        promised_events: Rc<Cell<FileEvent>>,
    ) -> Self {
        Self {
            sess,
            rgate,
            sgate,
            notify_events: FileEvent::empty(),
            pending_events: FileEvent::empty(),
            promised_events,
        }
    }

    pub fn send_events(&mut self) {
        let sg = self.sgate.get().unwrap();
        if !self.pending_events.is_empty() && sg.credits().unwrap() > 0 {
            log!(
                LogFlags::PipeData,
                "[{}] pipes::notify({:?})",
                self.sess,
                self.pending_events
            );
            // ignore errors
            send_vmsg!(sg, &self.rgate, self.pending_events.bits()).ok();
            // we promise the client that these operations will not block on the next call
            self.promised_events.set(self.pending_events);
            self.notify_events &= !self.pending_events;
            self.pending_events = FileEvent::empty();
        }
    }
}

bitflags! {
    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    pub struct Flags : u64 {
        const WRITE_EOF = 0x1;
        const READ_EOF  = 0x2;
    }
}

pub struct PendingRequest {
    chan: SessId,
    msg: &'static Message,
}

impl PendingRequest {
    fn new(chan: SessId, msg: &'static Message) -> Self {
        PendingRequest { chan, msg }
    }
}

pub struct State {
    flags: Flags,
    mem: Option<MemCap>,
    mem_size: usize,
    pub rbuf: VarRingBuf,
    pub last_read: Option<(SessId, usize)>,
    pub last_write: Option<(SessId, usize)>,
    pending_reads: Vec<PendingRequest>,
    pending_writes: Vec<PendingRequest>,
    notify_gates: Vec<NotifyGate>,
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
            notify_gates: Vec::new(),
            reader: Vec::new(),
            writer: Vec::new(),
        }
    }

    pub fn flags(&self) -> Flags {
        self.flags
    }

    pub fn add_flags(&mut self, flags: Flags) {
        self.flags |= flags;
    }

    pub fn has_pending_reads(&self) -> bool {
        !self.pending_reads.is_empty()
    }

    pub fn has_pending_writes(&self) -> bool {
        !self.pending_writes.is_empty()
    }

    pub fn get_read_size(&self) -> usize {
        assert!(!self.reader.is_empty());
        self.rbuf.size() / (4 * self.reader.len())
    }

    pub fn get_write_size(&self) -> usize {
        assert!(!self.writer.is_empty());
        self.rbuf.size() / (4 * self.writer.len())
    }

    pub fn get_notify_gate(&mut self, sess: SessId) -> Option<&mut NotifyGate> {
        self.notify_gates.iter_mut().find(|n| n.sess == sess)
    }

    fn receive_acks(&mut self) {
        for n in &mut self.notify_gates {
            if let Ok(msg) = n.rgate.fetch() {
                n.rgate.ack_msg(msg).unwrap();
                // try again to send events, if there are some
                n.send_events();
            }
        }
    }

    fn add_event(&mut self, event: FileEvent) {
        for n in &mut self.notify_gates {
            if n.notify_events.contains(event) {
                n.pending_events |= event;
                n.send_events();
            }
        }
    }

    pub fn enable_notify(
        &mut self,
        id: SessId,
        sgate: Selector,
        promised_events: Rc<Cell<FileEvent>>,
    ) -> Result<(), Error> {
        let rgate = RecvGate::new_with(RGateArgs::default().order(6).msg_order(6))?;

        self.notify_gates.push(NotifyGate::new(
            id,
            rgate,
            LazyGate::new(sgate),
            promised_events,
        ));
        Ok(())
    }

    pub fn request_notify(&mut self, id: SessId, events: FileEvent) -> Result<(), Error> {
        // we don't support multiple readers/writers in combination with notifications
        assert!(self.reader.len() <= 1 && self.writer.len() <= 1);
        let can_read = self.rbuf.get_read_pos(1).is_some();
        let can_write = self.rbuf.get_write_pos(1).is_some();
        let ng = self
            .get_notify_gate(id)
            .ok_or_else(|| Error::new(Code::NotSup))?;

        ng.notify_events |= events;
        // remove from promised events, because we need to notify the client about them again first
        ng.promised_events.set(ng.promised_events.get() & !events);
        if events.contains(FileEvent::INPUT) && can_read {
            ng.pending_events |= FileEvent::INPUT;
        }
        if events.contains(FileEvent::OUTPUT) && can_write {
            ng.pending_events |= FileEvent::OUTPUT;
        }
        ng.send_events();
        Ok(())
    }

    pub fn get_mem(&self, id: SessId, ty: ChanType, ep: Selector) -> Result<MemCap, Error> {
        // did we get a memory cap from the client?
        if let Some(mem) = &self.mem {
            // derive read-only/write-only mem cap
            let perm = match ty {
                ChanType::READ => kif::Perm::R,
                ChanType::WRITE => kif::Perm::W,
            };
            let cmem = mem.derive(0, self.mem_size, perm)?;
            // activate it on client's EP
            log!(
                LogFlags::PipeReqs,
                "[{}] pipes::activate(ep={}, gate={})",
                id,
                ep,
                cmem.sel()
            );
            EP::new_bind(0, ep).configure(cmem.sel())?;
            Ok(cmem)
        }
        else {
            Err(Error::new(Code::InvArgs))
        }
    }

    pub fn append_request(&mut self, id: SessId, is: &mut GateIStream<'_>, read: bool) {
        let req = PendingRequest::new(id, is.take_msg());
        if read {
            log!(LogFlags::PipeData, "[{}] pipes::read_wait()", id);
            self.pending_reads.insert(0, req);
        }
        else {
            log!(LogFlags::PipeData, "[{}] pipes::write_wait()", id);
            self.pending_writes.insert(0, req);
        }
    }

    pub fn handle_pending_reads(&mut self, rgate: &RecvGate) {
        self.receive_acks();

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
                    LogFlags::PipeData,
                    "[{}] pipes::late_read(): {} @ {}",
                    req.chan,
                    amount,
                    pos
                );
                reply_vmsg_late!(rgate, req.msg, Code::Success, pos, amount).ok();

                // remove write request
                self.pending_reads.pop();
                break;
            }
            // did all writers leave?
            else if self.flags.contains(Flags::WRITE_EOF) {
                // report EOF
                log!(LogFlags::PipeData, "[{}] pipes::late_read(): EOF", req.chan);
                reply_vmsg_late!(rgate, req.msg, Code::Success, 0usize, 0usize).ok();

                // remove write request
                self.pending_reads.pop();
            }
            else {
                // otherwise, don't consider more read requests
                break;
            }
        }

        // if there is any chance to read something, notify all that are waiting for this event
        if !self.notify_gates.is_empty() && self.rbuf.get_read_pos(1).is_some() {
            self.add_event(FileEvent::INPUT);
        }
    }

    pub fn handle_pending_writes(&mut self, rgate: &RecvGate) {
        self.receive_acks();

        // if a write is still in progress, we cannot start other writes
        if self.last_write.is_some() {
            return;
        }

        // if all readers left, just report EOF to all pending write requests
        if self.flags.contains(Flags::READ_EOF) {
            while let Some(req) = self.pending_writes.pop() {
                log!(
                    LogFlags::PipeData,
                    "[{}] pipes::late_write(): EOF",
                    req.chan
                );
                reply_vmsg_late!(rgate, req.msg, Code::EndOfFile).ok();
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
                    LogFlags::PipeData,
                    "[{}] pipes::late_write(): {} @ {}",
                    req.chan,
                    amount,
                    pos
                );
                reply_vmsg_late!(rgate, req.msg, Code::Success, pos, amount).ok();

                // remove write request
                self.pending_writes.pop();
            }
        }

        // if there is any chance to write something, notify all that are waiting for this event
        if !self.notify_gates.is_empty() && self.rbuf.get_write_pos(1).is_some() {
            self.add_event(FileEvent::OUTPUT);
        }
    }

    pub fn remove_pending(&mut self, read: bool, chan: SessId) {
        let list = if read {
            &mut self.pending_reads
        }
        else {
            &mut self.pending_writes
        };
        list.retain(|req| req.chan != chan);
    }

    pub fn remove_reader(&mut self, id: SessId) -> usize {
        let pos = self.reader.iter().position(|x| *x == id).unwrap();
        self.reader.remove(pos);
        self.reader.len()
    }

    pub fn remove_writer(&mut self, id: SessId) -> usize {
        let pos = self.writer.iter().position(|x| *x == id).unwrap();
        self.writer.remove(pos);
        self.writer.len()
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
        self.state.borrow_mut().mem = Some(MemCap::new_bind(sel));
    }

    pub fn new_chan(&self, sid: SessId, ty: ChanType) -> Result<Channel, Error> {
        Channel::new(sid, ty, self.id, self.state.clone())
    }

    pub fn attach(&mut self, chan: &Channel) {
        assert!(chan.pipe() == self.id);
        match chan.ty() {
            ChanType::READ => self.state.borrow_mut().reader.push(chan.id()),
            ChanType::WRITE => self.state.borrow_mut().writer.push(chan.id()),
        }
    }

    pub fn close(&mut self, sids: &mut Vec<SessId>) -> Result<(), Error> {
        let state = self.state.borrow();
        sids.extend_from_slice(&state.reader);
        sids.extend_from_slice(&state.writer);
        Ok(())
    }
}
