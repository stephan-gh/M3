/*
 * Copyright (C) 2023 Nils Asmussen, Barkhausen Institut
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
use m3::cell::{RefMut, StaticRefCell};
use m3::col::Vec;
use m3::com::{GateIStream, MemGate, RecvGate, SendGate, EP};
use m3::errors::{Code, Error};
use m3::io::{LogFlags, Serial, Write};
use m3::kif;
use m3::log;
use m3::mem::GlobOff;
use m3::rc::Rc;
use m3::reply_vmsg;
use m3::server::SessId;
use m3::tcu::Message;
use m3::tiles::Activity;
use m3::vfs::{FileEvent, FileInfo, FileMode, TMode};
use m3::{build_vmsg, send_vmsg};

use crate::input;

pub const BUF_SIZE: usize = 256;

static TMP_BUF: StaticRefCell<[u8; BUF_SIZE]> = StaticRefCell::new([0u8; BUF_SIZE]);

#[derive(Debug)]
pub struct Channel {
    id: SessId,
    active: bool,
    writing: bool,
    ep: Option<Selector>,
    our_mem: Rc<MemGate>,
    notify_gates: Option<(RecvGate, SendGate)>,
    notify_events: FileEvent,
    pending_events: FileEvent,
    promised_events: FileEvent,
    pending_nextin: Option<&'static Message>,
    mem: MemGate,
    pos: usize,
    len: usize,
}

fn mem_off(id: SessId) -> GlobOff {
    id as GlobOff * BUF_SIZE as GlobOff
}

impl Channel {
    pub fn new(id: SessId, mem: Rc<MemGate>, writing: bool) -> Result<Self, Error> {
        let cmem = mem.derive(mem_off(id), BUF_SIZE, kif::Perm::RW)?;

        Ok(Channel {
            id,
            active: false,
            writing,
            ep: None,
            our_mem: mem,
            notify_gates: None,
            notify_events: FileEvent::empty(),
            pending_events: FileEvent::empty(),
            promised_events: FileEvent::empty(),
            pending_nextin: None,
            mem: cmem,
            pos: 0,
            len: 0,
        })
    }

    pub fn is_writing(&self) -> bool {
        self.writing
    }

    pub fn set_dest(&mut self, ep: Selector) {
        self.ep = Some(ep);
    }

    pub fn notify_gates(&self) -> Option<&(RecvGate, SendGate)> {
        self.notify_gates.as_ref()
    }

    pub fn set_notify_gates(&mut self, rgate: RecvGate) -> Result<Selector, Error> {
        if self.notify_gates.is_some() {
            return Err(Error::new(Code::Exists));
        }

        let sel = Activity::own().alloc_sel();
        self.notify_gates = Some((rgate, SendGate::new_bind(sel)));
        Ok(sel)
    }

    fn activate(&mut self) -> Result<(), Error> {
        if !self.active {
            let sel = self.ep.ok_or_else(|| Error::new(Code::InvArgs))?;
            EP::new_bind(0, sel).configure(self.mem.sel())?;
            self.active = true;
        }
        Ok(())
    }

    pub fn get_tmode(&mut self, is: &mut GateIStream<'_>) -> Result<(), Error> {
        let _fid: usize = is.pop()?;

        log!(LogFlags::VTReqs, "[{}] vterm::get_tmode()", self.id,);

        reply_vmsg!(is, Code::Success, input::mode())
    }

    pub fn set_tmode(&mut self, is: &mut GateIStream<'_>) -> Result<(), Error> {
        let _fid: usize = is.pop()?;
        let mode = is.pop::<TMode>()?;

        log!(
            LogFlags::VTReqs,
            "[{}] vterm::set_tmode(mode={:?})",
            self.id,
            mode
        );
        input::set_mode(mode);

        is.reply_error(Code::Success)
    }

    pub fn next_in(&mut self, is: &mut GateIStream<'_>) -> Result<(), Error> {
        let _: usize = is.pop()?;

        log!(LogFlags::VTInOut, "[{}] vterm::next_in()", self.id);

        if self.writing {
            return Err(Error::new(Code::NoPerm));
        }

        self.pos += self.len - self.pos;

        self.activate()?;

        if self.pos == self.len {
            assert!(self.pending_nextin.is_none());

            let mut input = input::get();
            if !input::eof() && input.is_empty() {
                // if we promised the client that input would be available, report WouldBlock
                // instead of delaying the response.
                if self.promised_events.contains(FileEvent::INPUT) {
                    return Err(Error::new(Code::WouldBlock));
                }

                self.pending_nextin = Some(is.take_msg());
                return Ok(());
            }

            self.pending_nextin = Some(is.take_msg());
            self.fetch_input(&mut input)?;
        }

        reply_vmsg!(is, Code::Success, self.pos, self.len - self.pos)
    }

    pub fn fetch_input(
        &mut self,
        input: &mut RefMut<'_, Vec<u8>>,
    ) -> Result<Option<(&'static Message, usize, usize)>, Error> {
        if let Some(msg) = self.pending_nextin.take() {
            // okay, input is available, so we fulfilled our promise
            self.promised_events &= !FileEvent::INPUT;
            self.our_mem.write(input, mem_off(self.id))?;
            self.len = input.len();
            self.pos = 0;

            log!(
                LogFlags::VTInOut,
                "[{}] vterm::next_in() -> ({}, {})",
                self.id,
                self.pos,
                self.len - self.pos
            );

            input::set_eof(false);
            input.clear();

            Ok(Some((msg, self.pos, self.len)))
        }
        else {
            Ok(None)
        }
    }

    pub fn next_out(&mut self, is: &mut GateIStream<'_>) -> Result<(), Error> {
        let _: usize = is.pop()?;

        log!(LogFlags::VTInOut, "[{}] vterm::next_out()", self.id);

        if !self.writing {
            return Err(Error::new(Code::NoPerm));
        }

        self.flush(self.len)?;
        self.activate()?;

        self.pos = 0;
        self.len = BUF_SIZE;

        reply_vmsg!(is, Code::Success, 0usize, BUF_SIZE)
    }

    pub fn commit(&mut self, is: &mut GateIStream<'_>) -> Result<(), Error> {
        let _fid: usize = is.pop()?;
        let nbytes: usize = is.pop()?;

        log!(
            LogFlags::VTInOut,
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

        is.reply_error(Code::Success)
    }

    pub fn stat(&mut self, is: &mut GateIStream<'_>) -> Result<(), Error> {
        let info = FileInfo {
            mode: FileMode::IFCHR | FileMode::IRUSR | FileMode::IWUSR,
            ..Default::default()
        };

        let mut reply = m3::mem::MsgBuf::borrow_def();
        build_vmsg!(reply, Code::Success, info);
        is.reply(&reply)
    }

    fn flush(&mut self, nbytes: usize) -> Result<(), Error> {
        if nbytes > 0 {
            self.our_mem
                .read(&mut TMP_BUF.borrow_mut()[0..nbytes], mem_off(self.id))?;
            Serial::new().write(&TMP_BUF.borrow()[0..nbytes])?;
        }
        self.len = 0;
        Ok(())
    }

    pub fn request_notify(&mut self, is: &mut GateIStream<'_>) -> Result<(), Error> {
        let _: usize = is.pop()?;
        let events: FileEvent = FileEvent::from_bits_truncate(is.pop()?);

        log!(
            LogFlags::VTReqs,
            "[{}] vterm::req_notify(events={:?})",
            self.id,
            events
        );

        if self.notify_gates.is_none() {
            return Err(Error::new(Code::NotSup));
        }

        self.notify_events |= events;
        // remove from promised events, because we need to notify the client about them again first
        self.promised_events &= !events;
        // check whether input is available already
        if events.contains(FileEvent::INPUT) && !input::get().is_empty() {
            self.pending_events |= FileEvent::INPUT;
        }
        // output is always possible
        if events.contains(FileEvent::OUTPUT) {
            self.pending_events |= FileEvent::OUTPUT;
        }
        // directly notify the client, if there is any input or output possible
        self.send_events();

        is.reply_error(Code::Success)
    }

    pub fn add_event(&mut self, event: FileEvent) -> bool {
        if self.notify_events.contains(event) {
            log!(
                LogFlags::VTEvents,
                "[{}] vterm::received_event({:?})",
                self.id,
                event
            );
            self.pending_events |= event;
            self.send_events();
            true
        }
        else {
            false
        }
    }

    pub fn send_events(&mut self) {
        if !self.pending_events.is_empty() {
            let (rg, sg) = self.notify_gates.as_ref().unwrap();
            if sg.credits().unwrap() > 0 {
                log!(
                    LogFlags::VTEvents,
                    "[{}] vterm::sending_events({:?})",
                    self.id,
                    self.pending_events
                );
                // ignore errors
                send_vmsg!(sg, rg, self.pending_events.bits()).ok();
                // we promise the client that these operations will not block on the next call
                self.promised_events = self.pending_events;
                self.notify_events &= !self.pending_events;
                self.pending_events = FileEvent::empty();
            }
        }
    }
}
