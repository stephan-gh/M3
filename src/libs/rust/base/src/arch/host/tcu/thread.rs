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

use arch::envdata;
use arch::tcu::{
    backend, CmdReg, Command, Control, EpId, EpReg, Header, PEId, Reg, EP_COUNT, MAX_MSG_SIZE, TCU,
    UNLIM_CREDITS,
};
use cell::StaticCell;
use core::{intrinsics, ptr, sync::atomic};
use errors::{Code, Error};
use io;
use util;

pub(crate) struct Buffer {
    pub header: Header,
    pub data: [u8; MAX_MSG_SIZE],
}

impl Buffer {
    const fn new() -> Buffer {
        Buffer {
            header: Header::new(),
            data: [0u8; MAX_MSG_SIZE],
        }
    }

    fn as_words(&self) -> &[u64] {
        unsafe {
            #[allow(clippy::cast_ptr_alignment)]
            util::slice_for(
                self.data.as_ptr() as *const u64,
                MAX_MSG_SIZE / util::size_of::<u64>(),
            )
        }
    }

    fn as_words_mut(&mut self) -> &mut [u64] {
        unsafe {
            #[allow(clippy::cast_ptr_alignment)]
            util::slice_for_mut(
                self.data.as_mut_ptr() as *mut u64,
                MAX_MSG_SIZE / util::size_of::<u64>(),
            )
        }
    }
}

static LOG: StaticCell<Option<io::log::Log>> = StaticCell::new(None);
static BUFFER: StaticCell<Buffer> = StaticCell::new(Buffer::new());

fn log() -> &'static mut io::log::Log {
    LOG.get_mut().as_mut().unwrap()
}

fn buffer() -> &'static mut Buffer {
    BUFFER.get_mut()
}

macro_rules! log_tcu {
    ($fmt:expr)              => (log_tcu_impl!(TCU, concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => (log_tcu_impl!(TCU, concat!($fmt, "\n"), $($arg)*));
}

macro_rules! log_tcu_err {
    ($fmt:expr)              => (log_tcu_impl!(TCU_ERR, concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => (log_tcu_impl!(TCU_ERR, concat!($fmt, "\n"), $($arg)*));
}

macro_rules! log_tcu_impl {
    ($flag:tt, $($args:tt)*) => ({
        if $crate::io::log::$flag {
            #[allow(unused_imports)]
            use $crate::io::Write;
            $crate::arch::tcu::thread::log().write_fmt(format_args!($($args)*)).unwrap();
        }
    });
}

fn is_bit_set(mask: Reg, idx: u64) -> bool {
    (mask & (1 << idx)) != 0
}

fn set_bit(mask: Reg, idx: u64, val: bool) -> Reg {
    if val {
        mask | (1 << idx)
    }
    else {
        mask & !(1 << idx)
    }
}

fn prepare_send(ep: EpId) -> Result<(PEId, EpId), Error> {
    let msg = TCU::get_cmd(CmdReg::ADDR);
    let msg_size = TCU::get_cmd(CmdReg::SIZE) as usize;
    let credits = TCU::get_ep(ep, EpReg::CREDITS) as usize;

    // check if we have enough credits
    if credits != UNLIM_CREDITS as usize {
        let msg_order = TCU::get_ep(ep, EpReg::MSGORDER);
        if msg_order == 0 {
            log_tcu_err!("TCU-error: invalid EP {}", ep);
            return Err(Error::new(Code::InvEP));
        }

        let needed = 1 << msg_order;
        if needed > credits {
            log_tcu_err!(
                "TCU-error: insufficient credits on ep {} (have {:#x}, need {:#x})",
                ep,
                credits,
                needed
            );
            return Err(Error::new(Code::MissCredits));
        }

        TCU::set_ep(ep, EpReg::CREDITS, (credits - needed) as Reg);
    }

    let buf = buffer();
    buf.header.credits = 0;
    buf.header.label = TCU::get_ep(ep, EpReg::LABEL);

    // message
    buf.header.length = msg_size;
    unsafe {
        buf.data[0..msg_size].copy_from_slice(util::slice_for(msg as *const u8, msg_size));
    }

    Ok((
        TCU::get_ep(ep, EpReg::PE_ID) as PEId,
        TCU::get_ep(ep, EpReg::EP_ID) as EpId,
    ))
}

fn prepare_reply(ep: EpId) -> Result<(PEId, EpId), Error> {
    let src = TCU::get_cmd(CmdReg::ADDR);
    let size = TCU::get_cmd(CmdReg::SIZE) as usize;
    let reply = TCU::get_cmd(CmdReg::OFFSET) as usize;
    let buf_addr = TCU::get_ep(ep, EpReg::BUF_ADDR) as usize;
    let ord = TCU::get_ep(ep, EpReg::BUF_ORDER);
    let msg_ord = TCU::get_ep(ep, EpReg::BUF_MSGORDER);

    let idx = (reply - buf_addr) >> msg_ord;
    if idx >= (1 << (ord - msg_ord)) {
        log_tcu_err!("TCU-error: EP{}: invalid message addr {:#x}", ep, reply);
        return Err(Error::new(Code::InvArgs));
    }

    let reply_header: &Header = unsafe { intrinsics::transmute(reply) };
    if reply_header.has_replycap == 0 {
        log_tcu_err!("TCU-error: EP{}: double-reply for msg {:#x}?", ep, reply);
        return Err(Error::new(Code::InvArgs));
    }

    // ack message
    let mut occupied = TCU::get_ep(ep, EpReg::BUF_OCCUPIED);
    assert!(is_bit_set(occupied, idx as u64));
    occupied = set_bit(occupied, idx as u64, false);
    TCU::set_ep(ep, EpReg::BUF_OCCUPIED, occupied);
    log_tcu!("EP{}: acked message at index {}", ep, idx);

    let buf = buffer();
    buf.header.label = reply_header.reply_label;
    buf.header.credits = 1;
    buf.header.crd_ep = reply_header.snd_ep;
    // invalidate message for replying
    buf.header.has_replycap = 0;

    // message
    buf.header.length = size;
    unsafe {
        buf.data[0..size].copy_from_slice(util::slice_for(src as *const u8, size));
    }

    Ok((reply_header.pe as PEId, reply_header.rpl_ep as EpId))
}

fn check_rdwr(ep: EpId, read: bool) -> Result<(), Error> {
    let op = if read { 0 } else { 1 };
    let perms = TCU::get_ep(ep, EpReg::PERM);
    let credits = TCU::get_ep(ep, EpReg::CREDITS);
    let offset = TCU::get_cmd(CmdReg::OFFSET);
    let length = TCU::get_cmd(CmdReg::LENGTH);

    if (perms & (1 << op)) == 0 {
        log_tcu_err!(
            "TCU-error: EP{}: operation not permitted (perms={}, op={})",
            ep,
            perms,
            op
        );
        Err(Error::new(Code::NoPerm))
    }
    else {
        let end = offset.overflowing_add(length);
        if end.1 || end.0 > credits {
            log_tcu_err!(
                "TCU-error: EP{}: invalid parameters (credits={}, offset={}, datalen={})",
                ep,
                credits,
                offset,
                length
            );
            Err(Error::new(Code::InvArgs))
        }
        else {
            Ok(())
        }
    }
}

fn prepare_read(ep: EpId) -> Result<(PEId, EpId), Error> {
    check_rdwr(ep, true)?;

    let buf = buffer();

    buf.header.credits = 0;
    buf.header.label = TCU::get_ep(ep, EpReg::LABEL);
    buf.header.length = 3 * util::size_of::<u64>();

    let data = buf.as_words_mut();
    data[0] = TCU::get_cmd(CmdReg::OFFSET);
    data[1] = TCU::get_cmd(CmdReg::LENGTH);
    data[2] = TCU::get_cmd(CmdReg::ADDR);

    Ok((
        TCU::get_ep(ep, EpReg::PE_ID) as PEId,
        TCU::get_ep(ep, EpReg::EP_ID) as EpId,
    ))
}

fn prepare_write(ep: EpId) -> Result<(PEId, EpId), Error> {
    check_rdwr(ep, false)?;

    let buf = buffer();
    let src = TCU::get_cmd(CmdReg::ADDR);
    let size = TCU::get_cmd(CmdReg::SIZE) as usize;

    buf.header.credits = 0;
    buf.header.label = TCU::get_ep(ep, EpReg::LABEL);
    buf.header.length = size + 2 * util::size_of::<u64>();

    let data = buf.as_words_mut();
    data[0] = TCU::get_cmd(CmdReg::OFFSET);
    data[1] = size as u64;

    unsafe {
        libc::memcpy(
            data[2..].as_mut_ptr() as *mut libc::c_void,
            src as *const libc::c_void,
            size as usize,
        );
    }

    Ok((
        TCU::get_ep(ep, EpReg::PE_ID) as PEId,
        TCU::get_ep(ep, EpReg::EP_ID) as EpId,
    ))
}

fn prepare_ack(ep: EpId) -> Result<(PEId, EpId), Error> {
    let addr = TCU::get_cmd(CmdReg::OFFSET);
    let buf_addr = TCU::get_ep(ep, EpReg::BUF_ADDR);
    let msg_ord = TCU::get_ep(ep, EpReg::BUF_MSGORDER);
    let ord = TCU::get_ep(ep, EpReg::BUF_ORDER);

    let idx = (addr - buf_addr) >> msg_ord;
    if idx >= (1 << (ord - msg_ord)) {
        log_tcu_err!("TCU-error: EP{}: invalid message addr {:#x}", ep, addr);
        return Err(Error::new(Code::InvArgs));
    }

    let mut occupied = TCU::get_ep(ep, EpReg::BUF_OCCUPIED);
    let unread = TCU::get_ep(ep, EpReg::BUF_UNREAD);
    assert!(is_bit_set(occupied, idx));
    occupied = set_bit(occupied, idx, false);
    if is_bit_set(unread, idx) {
        let unread = set_bit(unread, idx, false);
        TCU::set_ep(ep, EpReg::BUF_UNREAD, unread);
        TCU::set_ep(
            ep,
            EpReg::BUF_MSG_CNT,
            TCU::get_ep(ep, EpReg::BUF_MSG_CNT) - 1,
        );
    }
    TCU::set_ep(ep, EpReg::BUF_OCCUPIED, occupied);

    log_tcu!("EP{}: acked message at index {}", ep, idx);

    Ok((0, EP_COUNT))
}

fn prepare_fetch(ep: EpId) -> Result<(PEId, EpId), Error> {
    let msgs = TCU::get_ep(ep, EpReg::BUF_MSG_CNT);
    if msgs == 0 {
        return Ok((0, EP_COUNT));
    }

    let unread = TCU::get_ep(ep, EpReg::BUF_UNREAD);
    let roff = TCU::get_ep(ep, EpReg::BUF_ROFF);
    let ord = TCU::get_ep(ep, EpReg::BUF_ORDER);
    let msg_ord = TCU::get_ep(ep, EpReg::BUF_MSGORDER);
    let size = 1 << (ord - msg_ord);

    let recv_msg = |idx| {
        assert!(is_bit_set(TCU::get_ep(ep, EpReg::BUF_OCCUPIED), idx));

        let unread = set_bit(unread, idx, false);
        let msgs = msgs - 1;
        assert!(unread.count_ones() == msgs as u32);

        log_tcu!("EP{}: fetched msg at index {} (count={})", ep, idx, msgs);

        TCU::set_ep(ep, EpReg::BUF_UNREAD, unread);
        TCU::set_ep(ep, EpReg::BUF_ROFF, idx + 1);
        TCU::set_ep(ep, EpReg::BUF_MSG_CNT, msgs);

        let addr = TCU::get_ep(ep, EpReg::BUF_ADDR);
        TCU::set_cmd(CmdReg::OFFSET, addr + idx * (1 << msg_ord));

        Ok((0, EP_COUNT))
    };

    for i in roff..size {
        if is_bit_set(unread, i) {
            return recv_msg(i);
        }
    }
    for i in 0..roff {
        if is_bit_set(unread, i) {
            return recv_msg(i);
        }
    }

    unreachable!();
}

fn handle_msg(ep: EpId, len: usize) {
    let msg_ord = TCU::get_ep(ep, EpReg::BUF_MSGORDER);
    let msg_size = 1 << msg_ord;
    if len > msg_size {
        log_tcu_err!(
            "TCU-error: dropping msg due to insufficient space (required: {}, available: {})",
            len,
            msg_size
        );
        return;
    }

    let occupied = TCU::get_ep(ep, EpReg::BUF_OCCUPIED);
    let woff = TCU::get_ep(ep, EpReg::BUF_WOFF);
    let ord = TCU::get_ep(ep, EpReg::BUF_ORDER);
    let size = 1 << (ord - msg_ord);

    let place_msg = |idx, occupied| {
        let unread = TCU::get_ep(ep, EpReg::BUF_UNREAD);
        let msgs = TCU::get_ep(ep, EpReg::BUF_MSG_CNT);

        let occupied = set_bit(occupied, idx, true);
        let unread = set_bit(unread, idx, true);
        let msgs = msgs + 1;
        assert!(unread.count_ones() == msgs as u32);

        log_tcu!("EP{}: put msg at index {} (count={})", ep, idx, msgs);

        TCU::set_ep(ep, EpReg::BUF_OCCUPIED, occupied);
        TCU::set_ep(ep, EpReg::BUF_UNREAD, unread);
        TCU::set_ep(ep, EpReg::BUF_MSG_CNT, msgs);
        TCU::set_ep(ep, EpReg::BUF_WOFF, idx + 1);

        let addr = TCU::get_ep(ep, EpReg::BUF_ADDR);
        let dst = (addr + idx * (1 << msg_ord)) as *mut u8;
        let src = &buffer().header as *const Header as *const u8;
        unsafe {
            util::slice_for_mut(dst, len).copy_from_slice(util::slice_for(src, len));
        }
    };

    for i in woff..size {
        if !is_bit_set(occupied, i) {
            place_msg(i, occupied);
            return;
        }
    }
    for i in 0..woff {
        if !is_bit_set(occupied, i) {
            place_msg(i, occupied);
            return;
        }
    }

    log_tcu_err!("TCU-error: EP{}: dropping msg because no slot is free", ep);
}

fn handle_write_cmd(backend: &backend::SocketBackend, ep: EpId) -> Result<(), Error> {
    let buf = buffer();
    let base = buf.header.label;

    {
        let data = buf.as_words();
        let offset = base + data[0];
        let length = data[1];

        log_tcu!(
            "(write) {} bytes to {:#x}+{:#x}",
            length,
            base,
            offset - base
        );
        assert!(length as usize <= MAX_MSG_SIZE - 2 * util::size_of::<u64>());

        unsafe {
            libc::memcpy(
                offset as *mut libc::c_void,
                data[2..].as_ptr() as *const libc::c_void,
                length as usize,
            );
        }
    }

    let dst_pe = buf.header.pe as PEId;
    let dst_ep = buf.header.rpl_ep as EpId;

    buf.header.opcode = Command::RESP.val as u8;
    buf.header.credits = 0;
    buf.header.label = 0;
    buf.header.length = 0;

    send_msg(backend, ep, dst_pe, dst_ep)
}

fn handle_read_cmd(backend: &backend::SocketBackend, ep: EpId) -> Result<(), Error> {
    let buf = buffer();
    let base = buf.header.label;

    let (offset, length, dest) = {
        let data = buf.as_words();
        (base + data[0], data[1], data[2])
    };

    log_tcu!(
        "(read) {} bytes from {:#x}+{:#x} -> {:#x}",
        length,
        base,
        offset - base,
        dest
    );
    assert!(length as usize <= MAX_MSG_SIZE - 3 * util::size_of::<u64>());

    let dst_pe = buf.header.pe as PEId;
    let dst_ep = buf.header.rpl_ep as EpId;

    buf.header.opcode = Command::RESP.val as u8;
    buf.header.credits = 0;
    buf.header.label = 0;
    buf.header.length = length as usize + 3 * util::size_of::<u64>();

    let data = buf.as_words_mut();
    data[0] = dest;
    data[1] = length;
    data[2] = 0;

    unsafe {
        libc::memcpy(
            data[3..].as_mut_ptr() as *mut libc::c_void,
            offset as *const libc::c_void,
            length as usize,
        );
    }

    send_msg(backend, ep, dst_pe, dst_ep)
}

fn handle_resp_cmd() {
    let buf = buffer();
    let data = buf.as_words();
    let base = buf.header.label;
    let resp = if buf.header.length > 0 {
        let offset = base + data[0];
        let length = data[1];
        let resp = data[2];

        log_tcu!(
            "(resp) {} bytes to {:#x}+{:#x} -> {:#x}",
            length,
            base,
            offset - base,
            resp
        );
        assert!(length as usize <= MAX_MSG_SIZE - 3 * util::size_of::<usize>());

        unsafe {
            libc::memcpy(
                offset as *mut libc::c_void,
                data[3..].as_ptr() as *const libc::c_void,
                length as usize,
            );
        }
        resp
    }
    else {
        0
    };

    // provide feedback to SW
    TCU::set_cmd(CmdReg::CTRL, resp << 16);
}

#[rustfmt::skip]
fn send_msg(
    backend: &backend::SocketBackend,
    ep: EpId,
    dst_pe: PEId,
    dst_ep: EpId,
) -> Result<(), Error> {
    let buf = buffer();

    log_tcu!(
        "{} {:3}b lbl={:#016x} over {} to pe:ep={}:{} (crd={:#x} rep={})",
        if buf.header.opcode == Command::REPLY.val as u8 { ">>" } else { "->" },
        { buf.header.length },
        { buf.header.label },
        buf.header.snd_ep,
        dst_pe,
        dst_ep,
        TCU::get_ep(ep, EpReg::CREDITS),
        buf.header.rpl_ep
    );

    if backend.send(dst_pe, dst_ep, buf) {
        Ok(())
    }
    else {
        Err(Error::new(Code::RecvGone))
    }
}

fn handle_command(backend: &backend::SocketBackend) {
    // clear error
    TCU::set_cmd(CmdReg::CTRL, TCU::get_cmd(CmdReg::CTRL) & 0xFFFF);

    let ep = TCU::get_cmd(CmdReg::EPID) as EpId;

    let res = if ep >= EP_COUNT {
        log_tcu_err!("TCU-error: invalid ep-id ({})", ep);
        Err(Error::new(Code::InvArgs))
    }
    else {
        let ctrl = TCU::get_cmd(CmdReg::CTRL);
        let op: Command = Command::from((ctrl >> 3) & 0xF);

        let res = match op {
            Command::SEND => prepare_send(ep),
            Command::REPLY => prepare_reply(ep),
            Command::READ => prepare_read(ep),
            Command::WRITE => prepare_write(ep),
            Command::FETCH_MSG => prepare_fetch(ep),
            Command::ACK_MSG => prepare_ack(ep),
            _ => Err(Error::new(Code::NotSup)),
        };

        match res {
            Ok((dst_pe, dst_ep)) if dst_ep < EP_COUNT => {
                let buf = buffer();
                buf.header.opcode = op.val as u8;

                if op != Command::REPLY {
                    // reply cap
                    buf.header.has_replycap = 1;
                    buf.header.pe = envdata::get().pe_id as u16;
                    buf.header.snd_ep = ep as u8;
                    buf.header.rpl_ep = TCU::get_cmd(CmdReg::REPLY_EPID) as u8;
                    buf.header.reply_label = TCU::get_cmd(CmdReg::REPLY_LBL);
                }

                match send_msg(backend, ep, dst_pe, dst_ep) {
                    Err(e) => Err(e),
                    Ok(_) => {
                        if op == Command::READ || op == Command::WRITE {
                            // wait for the response
                            Ok(op.val << 3)
                        }
                        else {
                            Ok(0)
                        }
                    },
                }
            },
            Ok((_, _)) => Ok(0),
            Err(e) => Err(e),
        }
    };

    match res {
        Ok(val) => TCU::set_cmd(CmdReg::CTRL, val),
        Err(e) => TCU::set_cmd(CmdReg::CTRL, (e.code() as Reg) << 16),
    };
}

fn handle_receive(backend: &backend::SocketBackend, ep: EpId) -> bool {
    let buf = buffer();
    if let Some(size) = backend.receive(ep, buf) {
        match Command::from(buf.header.opcode) {
            Command::SEND | Command::REPLY => handle_msg(ep, size),
            Command::READ => handle_read_cmd(backend, ep).unwrap(),
            Command::WRITE => handle_write_cmd(backend, ep).unwrap(),
            Command::RESP => handle_resp_cmd(),
            _ => panic!("Not supported!"),
        }

        // refill credits
        let crd_ep = buf.header.crd_ep as EpId;
        if crd_ep >= EP_COUNT {
            log_tcu_err!("TCU-error: should give credits to ep {}", crd_ep);
        }
        else {
            let msg_ord = TCU::get_ep(crd_ep, EpReg::MSGORDER);
            let credits = TCU::get_ep(crd_ep, EpReg::CREDITS);
            if buf.header.credits != 0 && credits != UNLIM_CREDITS as u64 {
                log_tcu!(
                    "Refilling credits of ep {} from {:#x} to {:#x}",
                    crd_ep,
                    credits,
                    credits + (1 << msg_ord)
                );
                TCU::set_ep(crd_ep, EpReg::CREDITS, credits + (1 << msg_ord));
            }
        }

        log_tcu!(
            "<- {:3}b lbl={:#016x} ep={} (cnt={:#x}, crd={:#x})",
            size - util::size_of::<Header>(),
            { buf.header.label },
            ep,
            TCU::get_ep(ep, EpReg::BUF_MSG_CNT),
            TCU::get_ep(ep, EpReg::CREDITS),
        );
        true
    }
    else {
        false
    }
}

static BACKEND: StaticCell<Option<backend::SocketBackend>> = StaticCell::new(None);
static RUN: atomic::AtomicUsize = atomic::AtomicUsize::new(1);
static mut TID: libc::pthread_t = 0;

extern "C" fn sigchild(_: i32) {
    // send notification to kernel
    unsafe {
        let mut status: i32 = 0;
        let pid = libc::wait(&mut status as *mut i32);
        if pid != -1 {
            BACKEND.get().as_ref().unwrap().notify_kernel(pid, status);
        }

        libc::signal(libc::SIGCHLD, sigchild as usize);
    }
}

extern "C" fn run(_arg: *mut libc::c_void) -> *mut libc::c_void {
    let backend = BACKEND.get_mut().as_mut().unwrap();

    unsafe {
        libc::signal(libc::SIGCHLD, sigchild as usize);
    }

    while RUN.load(atomic::Ordering::Relaxed) == 1 {
        if (TCU::get_cmd(CmdReg::CTRL) & Control::START.bits()) != 0 {
            handle_command(&backend);
        }

        for ep in 0..EP_COUNT {
            handle_receive(&backend, ep);
        }

        TCU::sleep().unwrap();
    }

    // deny further receives
    backend.shutdown();

    // handle all outstanding messages
    loop {
        let mut received = false;
        for ep in 0..EP_COUNT {
            received |= handle_receive(&backend, ep);
        }
        if !received {
            break;
        }

        TCU::sleep().unwrap();
    }

    ptr::null_mut()
}

pub fn init() {
    LOG.set(Some(io::log::Log::default()));
    log().init(envdata::get().pe_id, "TCU");

    BACKEND.set(Some(backend::SocketBackend::new()));

    unsafe {
        let res = libc::pthread_create(&mut TID, ptr::null(), run, ptr::null_mut());
        assert!(res == 0);
    }
}

pub fn deinit() {
    RUN.store(0, atomic::Ordering::Relaxed);

    unsafe {
        // libc::pthread_kill(TID, libc::SIGUSR1);
        assert!(libc::pthread_join(TID, ptr::null_mut()) == 0);
    }

    BACKEND.set(None);
}
