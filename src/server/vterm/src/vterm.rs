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

use m3::cap::Selector;
use m3::cell::{LazyStaticRefCell, RefMut, StaticCell, StaticRefCell};
use m3::col::Vec;
use m3::com::{opcodes, GateIStream, MemGate, Perm, RGateArgs, RecvGate, SendGate, EP};
use m3::errors::{Code, Error};
use m3::int_enum;
use m3::io::{LogFlags, Serial, Write};
use m3::kif;
use m3::log;
use m3::rc::Rc;
use m3::reply_vmsg;
use m3::server::{
    server_loop, CapExchange, ClientManager, ExcType, RequestHandler, RequestSession, Server,
    SessId, DEF_MAX_CLIENTS,
};
use m3::session::ServerSession;
use m3::tcu::Message;
use m3::tiles::Activity;
use m3::vec;
use m3::vfs::{FileEvent, FileInfo, FileMode};
use m3::{build_vmsg, goff, send_vmsg};

const BUF_SIZE: usize = 256;

int_enum! {
    struct Mode : u64 {
        const RAW       = 0;
        const COOKED    = 1;
    }
}

static SERV_SEL: StaticCell<Selector> = StaticCell::new(0);
static CLOSED_SESS: StaticRefCell<Option<SessId>> = StaticRefCell::new(None);

static MEM: LazyStaticRefCell<Rc<MemGate>> = LazyStaticRefCell::default();
static BUFFER: StaticRefCell<Vec<u8>> = StaticRefCell::new(Vec::new());
static INPUT: StaticRefCell<Vec<u8>> = StaticRefCell::new(Vec::new());
static EOF: StaticCell<bool> = StaticCell::new(false);
static MODE: StaticCell<Mode> = StaticCell::new(Mode::COOKED);
static TMP_BUF: StaticRefCell<[u8; BUF_SIZE]> = StaticRefCell::new([0u8; BUF_SIZE]);

macro_rules! reply_vmsg_late {
    ( $rgate:expr, $msg:expr, $( $args:expr ),* ) => ({
        let mut msg = m3::mem::MsgBuf::borrow_def();
        m3::build_vmsg!(&mut msg, $( $args ),*);
        $rgate.reply(&msg, $msg)
    });
}

#[derive(Debug)]
struct VTermSession {
    crt: usize,
    _serv: ServerSession,
    data: SessionData,
    parent: Option<SessId>,
    childs: Vec<SessId>,
}

#[allow(clippy::large_enum_variant)]
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

fn mem_off(id: SessId) -> goff {
    id as goff * BUF_SIZE as goff
}

impl Channel {
    fn new(id: SessId, mem: Rc<MemGate>, writing: bool) -> Result<Self, Error> {
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

    fn activate(&mut self) -> Result<(), Error> {
        if !self.active {
            let sel = self.ep.ok_or_else(|| Error::new(Code::InvArgs))?;
            EP::new_bind(0, sel).configure(self.mem.sel())?;
            self.active = true;
        }
        Ok(())
    }

    fn get_tmode(&mut self, is: &mut GateIStream<'_>) -> Result<(), Error> {
        let _fid: usize = is.pop()?;

        log!(LogFlags::VTReqs, "[{}] vterm::get_tmode()", self.id,);

        reply_vmsg!(is, Code::Success, MODE.get())
    }

    fn set_tmode(&mut self, is: &mut GateIStream<'_>) -> Result<(), Error> {
        let _fid: usize = is.pop()?;
        let mode = is.pop::<Mode>()?;

        log!(
            LogFlags::VTReqs,
            "[{}] vterm::set_tmode(mode={})",
            self.id,
            mode
        );
        MODE.set(mode);
        INPUT.borrow_mut().clear();

        is.reply_error(Code::Success)
    }

    fn next_in(&mut self, is: &mut GateIStream<'_>) -> Result<(), Error> {
        let _: usize = is.pop()?;

        log!(LogFlags::VTInOut, "[{}] vterm::next_in()", self.id);

        if self.writing {
            return Err(Error::new(Code::NoPerm));
        }

        self.pos += self.len - self.pos;

        self.activate()?;

        if self.pos == self.len {
            let mut input = INPUT.borrow_mut();
            if !EOF.get() && input.is_empty() {
                // if we promised the client that input would be available, report WouldBlock
                // instead of delaying the response.
                if self.promised_events.contains(FileEvent::INPUT) {
                    return Err(Error::new(Code::WouldBlock));
                }

                assert!(self.pending_nextin.is_none());
                self.pending_nextin = Some(is.take_msg());
                return Ok(());
            }

            self.fetch_input(&mut input)?;
        }

        reply_vmsg!(is, Code::Success, self.pos, self.len - self.pos)
    }

    fn fetch_input(&mut self, input: &mut RefMut<'_, Vec<u8>>) -> Result<(), Error> {
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

        EOF.set(false);
        input.clear();

        Ok(())
    }

    fn next_out(&mut self, is: &mut GateIStream<'_>) -> Result<(), Error> {
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

    fn commit(&mut self, is: &mut GateIStream<'_>) -> Result<(), Error> {
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

    fn stat(&mut self, is: &mut GateIStream<'_>) -> Result<(), Error> {
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

    fn request_notify(&mut self, is: &mut GateIStream<'_>) -> Result<(), Error> {
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
        if events.contains(FileEvent::INPUT) && !INPUT.borrow().is_empty() {
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

    fn add_event(&mut self, event: FileEvent) -> bool {
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

    fn send_events(&mut self) {
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

impl RequestSession for VTermSession {
    fn new(crt: usize, _serv: ServerSession, _arg: &str) -> Result<Self, Error>
    where
        Self: Sized,
    {
        Ok(VTermSession {
            crt,
            _serv,
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
            if let Some(p) = cli.sessions_mut().get_mut(pid) {
                p.childs.retain(|cid| *cid != sid);
            }
        }
    }
}

impl VTermSession {
    fn get_sess(cli: &mut ClientManager<Self>, sid: SessId) -> Result<&mut Self, Error> {
        cli.sessions_mut()
            .get_mut(sid)
            .ok_or_else(|| Error::new(Code::InvArgs))
    }

    fn with_chan<F, R>(&mut self, is: &mut GateIStream<'_>, func: F) -> Result<R, Error>
    where
        F: Fn(&mut Channel, &mut GateIStream<'_>) -> Result<R, Error>,
    {
        match &mut self.data {
            SessionData::Meta => Err(Error::new(Code::InvArgs)),
            SessionData::Chan(c) => func(c, is),
        }
    }

    fn new_chan(
        parent: SessId,
        sess: Selector,
        crt: usize,
        sid: SessId,
        writing: bool,
    ) -> Result<VTermSession, Error> {
        log!(LogFlags::VTReqs, "[{}] vterm::new_chan()", sid);

        Ok(VTermSession {
            crt,
            _serv: ServerSession::new_with_sel(SERV_SEL.get(), sess, crt, sid as u64, false)?,
            data: SessionData::Chan(Channel::new(sid, MEM.borrow().clone(), writing)?),
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

        let sels = Activity::own().alloc_sels(2);
        cli.add_connected_session(crt, sels + 1, |cli, nsid, _sgate| {
            let parent_sess = Self::get_sess(cli, sid)?;

            let child_sess = match &parent_sess.data {
                SessionData::Meta => {
                    let writing = xchg.in_args().pop::<i32>()? == 1;
                    Self::new_chan(sid, sels, crt, nsid, writing)
                },

                SessionData::Chan(c) => Self::new_chan(sid, sels, crt, nsid, c.writing),
            }?;

            // remember that the new session is a child of the current one
            parent_sess.childs.push(nsid);
            Ok(child_sess)
        })?;

        xchg.out_caps(kif::CapRngDesc::new(kif::CapType::OBJECT, sels, 2));
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
                let sel = Activity::own().alloc_sel();
                c.ep = Some(sel);
                xchg.out_caps(kif::CapRngDesc::new(kif::CapType::OBJECT, sel, 1));
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
                if c.notify_gates.is_some() {
                    return Err(Error::new(Code::Exists));
                }

                let sel = Activity::own().alloc_sel();
                let rgate = RecvGate::new_with(RGateArgs::default().order(6).msg_order(6))?;
                rgate.activate()?;
                c.notify_gates = Some((rgate, SendGate::new_bind(sel)));
                xchg.out_caps(kif::CapRngDesc::new(kif::CapType::OBJECT, sel, 1));
                Ok(())
            },
            _ => Err(Error::new(Code::InvArgs)),
        }
    }
}

fn add_signal(cli: &mut ClientManager<VTermSession>) {
    cli.sessions_mut().for_each(|s| match &mut s.data {
        SessionData::Chan(c) => {
            c.add_event(FileEvent::SIGNAL);
        },
        SessionData::Meta => {},
    });
}

fn add_input(
    cli: &mut ClientManager<VTermSession>,
    eof: bool,
    mut flush: bool,
    input: &mut RefMut<'_, Vec<u8>>,
) {
    // pass to first session that wants input
    EOF.set(eof);

    let mut input_recv: Option<(&Message, usize, usize)> = None;

    cli.sessions_mut().for_each(|s| {
        if flush || !input.is_empty() {
            if let SessionData::Chan(c) = &mut s.data {
                let msg = c.pending_nextin.take();
                if let Some(msg) = msg {
                    c.fetch_input(input).unwrap();
                    input_recv = Some((msg, c.pos, c.len));
                    flush = false;
                }
                else if c.add_event(FileEvent::INPUT) {
                    flush = false;
                }
            }
        }
    });

    if let Some((msg, pos, len)) = input_recv {
        reply_vmsg_late!(cli.recv_gate(), msg, Code::Success, pos, len - pos).unwrap();
    }
}

fn receive_acks(cli: &mut ClientManager<VTermSession>) {
    cli.sessions_mut().for_each(|s| match &mut s.data {
        SessionData::Chan(c) => {
            if let Some((rg, _sg)) = &c.notify_gates {
                if let Ok(msg) = rg.fetch() {
                    rg.ack_msg(msg).unwrap();
                    // try again to send events, if there are some
                    c.send_events();
                }
            }
        },
        SessionData::Meta => {},
    });
}

fn handle_input(cli: &mut ClientManager<VTermSession>, msg: &'static Message) {
    let mut input = INPUT.borrow_mut();
    let mut buffer = BUFFER.borrow_mut();

    let bytes = unsafe { core::slice::from_raw_parts(msg.data.as_ptr(), msg.header.length()) };
    let mut flush = false;
    let mut eof = false;
    if MODE.get() == Mode::RAW {
        input.extend_from_slice(bytes);
    }
    else {
        let mut output = vec![];
        for b in bytes {
            match b {
                // ^D
                0x04 => eof = true,
                // ^C
                0x03 => add_signal(cli),
                // backspace
                0x7f => {
                    output.push(0x08);
                    output.push(b' ');
                    output.push(0x08);
                    buffer.pop();
                },
                b => {
                    if *b == 27 {
                        buffer.push(b'^');
                        output.push(b'^');
                    }
                    else if *b == b'\n' {
                        flush = true;
                    }
                    if *b == b'\n' || !b.is_ascii_control() {
                        buffer.push(*b);
                    }
                },
            }

            if *b == b'\n' || !b.is_ascii_control() {
                output.push(*b);
            }
        }

        if eof || flush {
            input.extend_from_slice(&buffer);
            buffer.clear();
        }
        Serial::new().write(&output).unwrap();
    }

    add_input(cli, eof, eof || flush, &mut input);
}

fn register_close(sid: SessId) {
    assert!(crate::CLOSED_SESS.borrow().is_none());
    *crate::CLOSED_SESS.borrow_mut() = Some(sid);
}

#[no_mangle]
pub fn main() -> Result<(), Error> {
    MEM.set(Rc::new(
        MemGate::new(DEF_MAX_CLIENTS * BUF_SIZE, Perm::RW).expect("Unable to alloc memory"),
    ));

    let mut hdl = RequestHandler::new().expect("Unable to create request handler");

    let srv = Server::new("vterm", &mut hdl).expect("Unable to create service 'vterm'");
    SERV_SEL.set(srv.sel());

    use opcodes::File;
    hdl.reg_cap_handler(File::CLONE.val, ExcType::Obt(2), VTermSession::clone);
    hdl.reg_cap_handler(File::SET_DEST.val, ExcType::Del(1), VTermSession::set_dest);
    hdl.reg_cap_handler(
        File::ENABLE_NOTIFY.val,
        ExcType::Del(1),
        VTermSession::enable_notify,
    );

    hdl.reg_msg_handler(File::NEXT_IN.val, |sess, is| {
        sess.with_chan(is, |c, is| c.next_in(is))
    });
    hdl.reg_msg_handler(File::NEXT_IN.val, |sess, is| {
        sess.with_chan(is, |c, is| c.next_in(is))
    });
    hdl.reg_msg_handler(File::NEXT_OUT.val, |sess, is| {
        sess.with_chan(is, |c, is| c.next_out(is))
    });
    hdl.reg_msg_handler(File::COMMIT.val, |sess, is| {
        sess.with_chan(is, |c, is| c.commit(is))
    });
    hdl.reg_msg_handler(File::STAT.val, |sess, is| {
        sess.with_chan(is, |c, is| c.stat(is))
    });
    hdl.reg_msg_handler(File::CLOSE.val, |_sess, is| {
        let sid = is.label() as SessId;
        is.reply_error(Code::Success).ok();
        register_close(sid);
        Ok(())
    });
    hdl.reg_msg_handler(File::SEEK.val, |_sess, _is| Err(Error::new(Code::NotSup)));
    hdl.reg_msg_handler(File::GET_TMODE.val, |sess, is| {
        sess.with_chan(is, |c, is| c.get_tmode(is))
    });
    hdl.reg_msg_handler(File::SET_TMODE.val, |sess, is| {
        sess.with_chan(is, |c, is| c.set_tmode(is))
    });
    hdl.reg_msg_handler(File::REQ_NOTIFY.val, |sess, is| {
        sess.with_chan(is, |c, is| c.request_notify(is))
    });

    let sel = Activity::own().alloc_sel();
    let serial_gate = Activity::own()
        .resmng()
        .unwrap()
        .get_serial(sel)
        .expect("Unable to allocate serial rgate");

    server_loop(|| {
        srv.fetch_and_handle(&mut hdl)?;

        if let Ok(msg) = serial_gate.fetch() {
            log!(
                LogFlags::VTInOut,
                "Got input message with {} bytes",
                msg.header.length()
            );
            handle_input(hdl.clients_mut(), msg);
            serial_gate.ack_msg(msg).unwrap();
        }

        receive_acks(hdl.clients_mut());

        hdl.fetch_and_handle()?;

        // check if there is a session to close
        if let Some(sid) = CLOSED_SESS.borrow_mut().take() {
            let creator = hdl.clients().sessions().get(sid).unwrap().crt;
            hdl.clients_mut().remove_session(creator, sid);
        }

        Ok(())
    })
    .ok();

    Ok(())
}
