/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
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

use core::{ptr, sync::atomic};

use crate::arch::envdata;
use crate::arch::tcu::{
    backend, CmdReg, Command, Control, EpId, EpReg, Header, Reg, TileId, MAX_MSG_SIZE, TCU,
    TOTAL_EPS, UNLIM_CREDITS, UNLIM_TIMEOUT,
};
use crate::cell::{LazyStaticRefCell, RefMut, StaticCell, StaticRefCell, StaticUnsafeCell};
use crate::errors::{Code, Error};
use crate::io;
use crate::mem;
use crate::util;

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
                MAX_MSG_SIZE / mem::size_of::<u64>(),
            )
        }
    }

    fn as_words_mut(&mut self) -> &mut [u64] {
        unsafe {
            #[allow(clippy::cast_ptr_alignment)]
            util::slice_for_mut(
                self.data.as_mut_ptr() as *mut u64,
                MAX_MSG_SIZE / mem::size_of::<u64>(),
            )
        }
    }
}

#[derive(Copy, Clone, Debug)]
enum SleepState {
    None,
    UntilMsg,
    UntilTimeout(u64),
}

pub(crate) static LOG: LazyStaticRefCell<io::log::Log> = LazyStaticRefCell::default();
static BUFFER: StaticRefCell<Buffer> = StaticRefCell::new(Buffer::new());
static MSG_CNT: StaticCell<usize> = StaticCell::new(0);
static SLEEP: StaticCell<SleepState> = StaticCell::new(SleepState::None);

#[macro_export]
macro_rules! log_tcu {
    ($fmt:expr)              => (crate::log_tcu_impl!(TCU, concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => (crate::log_tcu_impl!(TCU, concat!($fmt, "\n"), $($arg)*));
}

#[macro_export]
macro_rules! log_tcu_critical {
    ($fmt:expr)              => (crate::log_tcu_impl!(TCU_ERR, concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => (crate::log_tcu_impl!(TCU_ERR, concat!($fmt, "\n"), $($arg)*));
}

#[macro_export]
macro_rules! log_tcu_impl {
    ($flag:tt, $($args:tt)*) => ({
        if $crate::io::log::$flag {
            #[allow(unused_imports)]
            use $crate::io::Write;
            $crate::arch::tcu::thread::LOG.borrow_mut().write_fmt(format_args!($($args)*)).unwrap();
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

fn prepare_send(ep: EpId) -> Result<(TileId, EpId), Error> {
    let msg = TCU::get_cmd(CmdReg::ADDR);
    let msg_size = TCU::get_cmd(CmdReg::SIZE) as usize;
    let credits = TCU::get_ep(ep, EpReg::CREDITS) as usize;

    let msg_order = TCU::get_ep(ep, EpReg::MSGORDER);
    if msg_order == 0 {
        log_tcu!("TCU-error: invalid EP {}", ep);
        return Err(Error::new(Code::NoSEP));
    }

    // check if we have enough credits
    let needed = 1 << msg_order;
    if credits != UNLIM_CREDITS as usize {
        if needed > credits {
            log_tcu!(
                "TCU-error: insufficient credits on ep {} (have {:#x}, need {:#x})",
                ep,
                credits,
                needed
            );
            return Err(Error::new(Code::NoCredits));
        }

        TCU::set_ep(ep, EpReg::CREDITS, (credits - needed) as Reg);
    }

    // check if the message is small enough
    let total_msg_size = msg_size + mem::size_of::<Header>();
    if total_msg_size > needed {
        log_tcu!(
            "TCU-error: message too large for ep {} (max {:#x}, need {:#x})",
            ep,
            needed,
            total_msg_size
        );
        return Err(Error::new(Code::OutOfBounds));
    }

    let mut buf = BUFFER.borrow_mut();
    buf.header.credits = 0;
    buf.header.label = TCU::get_ep(ep, EpReg::LABEL);

    // message
    buf.header.length = msg_size;
    unsafe {
        buf.data[0..msg_size].copy_from_slice(util::slice_for(msg as *const u8, msg_size));
    }

    Ok((
        TCU::get_ep(ep, EpReg::TILE_ID) as TileId,
        TCU::get_ep(ep, EpReg::EP_ID) as EpId,
    ))
}

fn prepare_reply(ep: EpId) -> Result<(TileId, EpId), Error> {
    let src = TCU::get_cmd(CmdReg::ADDR);
    let size = TCU::get_cmd(CmdReg::SIZE) as usize;
    let reply_off = TCU::get_cmd(CmdReg::OFFSET) as usize;
    let buf_addr = TCU::get_ep(ep, EpReg::BUF_ADDR) as usize;
    let ord = TCU::get_ep(ep, EpReg::BUF_ORDER);
    let msg_ord = TCU::get_ep(ep, EpReg::BUF_MSGORDER);

    let idx = reply_off >> msg_ord;
    if idx >= (1 << (ord - msg_ord)) {
        log_tcu!(
            "TCU-error: EP{}: invalid message offset {:#x}",
            ep,
            reply_off
        );
        return Err(Error::new(Code::InvArgs));
    }

    let reply_msg = TCU::offset_to_msg(buf_addr, reply_off);
    if reply_msg.header.has_replycap == 0 {
        log_tcu!(
            "TCU-error: EP{}: double-reply for msg offset {:#x}?",
            ep,
            reply_off
        );
        return Err(Error::new(Code::InvArgs));
    }

    // ack message
    let mut occupied = TCU::get_ep(ep, EpReg::BUF_OCCUPIED);
    // if the slot is not occupied, it's equivalent to the reply EP being invalid
    if !is_bit_set(occupied, idx as u64) {
        return Err(Error::new(Code::NoSEP));
    }

    occupied = set_bit(occupied, idx as u64, false);
    TCU::set_ep(ep, EpReg::BUF_OCCUPIED, occupied);
    log_tcu!("EP{}: acked message at index {}", ep, idx);

    let mut buf = BUFFER.borrow_mut();
    buf.header.label = reply_msg.header.reply_label;
    buf.header.credits = 1;
    buf.header.crd_ep = reply_msg.header.snd_ep;
    // invalidate message for replying
    buf.header.has_replycap = 0;

    // message
    buf.header.length = size;
    unsafe {
        buf.data[0..size].copy_from_slice(util::slice_for(src as *const u8, size));
    }

    Ok((
        reply_msg.header.tile as TileId,
        reply_msg.header.rpl_ep as EpId,
    ))
}

fn check_rdwr(ep: EpId, read: bool) -> Result<(), Error> {
    let op = if read { 0 } else { 1 };
    let perms = TCU::get_ep(ep, EpReg::PERM);
    let credits = TCU::get_ep(ep, EpReg::CREDITS);
    let offset = TCU::get_cmd(CmdReg::OFFSET);
    let length = TCU::get_cmd(CmdReg::LENGTH);

    if (perms & (1 << op)) == 0 {
        log_tcu!(
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
            log_tcu!(
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

fn prepare_read(ep: EpId) -> Result<(TileId, EpId), Error> {
    check_rdwr(ep, true)?;

    let mut buf = BUFFER.borrow_mut();

    buf.header.credits = 0;
    buf.header.label = TCU::get_ep(ep, EpReg::LABEL);
    buf.header.length = 3 * mem::size_of::<u64>();

    let data = buf.as_words_mut();
    data[0] = TCU::get_cmd(CmdReg::OFFSET);
    data[1] = TCU::get_cmd(CmdReg::LENGTH);
    data[2] = TCU::get_cmd(CmdReg::ADDR);

    Ok((
        TCU::get_ep(ep, EpReg::TILE_ID) as TileId,
        TCU::get_ep(ep, EpReg::EP_ID) as EpId,
    ))
}

fn prepare_write(ep: EpId) -> Result<(TileId, EpId), Error> {
    check_rdwr(ep, false)?;

    let mut buf = BUFFER.borrow_mut();
    let src = TCU::get_cmd(CmdReg::ADDR);
    let size = TCU::get_cmd(CmdReg::SIZE) as usize;

    buf.header.credits = 0;
    buf.header.label = TCU::get_ep(ep, EpReg::LABEL);
    buf.header.length = size + 2 * mem::size_of::<u64>();

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
        TCU::get_ep(ep, EpReg::TILE_ID) as TileId,
        TCU::get_ep(ep, EpReg::EP_ID) as EpId,
    ))
}

fn prepare_ack(ep: EpId) -> Result<(TileId, EpId), Error> {
    let msg_off = TCU::get_cmd(CmdReg::OFFSET);
    let msg_ord = TCU::get_ep(ep, EpReg::BUF_MSGORDER);
    let ord = TCU::get_ep(ep, EpReg::BUF_ORDER);

    let idx = msg_off >> msg_ord;
    if idx >= (1 << (ord - msg_ord)) {
        log_tcu!("TCU-error: EP{}: invalid message offset {:#x}", ep, msg_off);
        return Err(Error::new(Code::InvArgs));
    }

    let mut occupied = TCU::get_ep(ep, EpReg::BUF_OCCUPIED);
    let unread = TCU::get_ep(ep, EpReg::BUF_UNREAD);
    occupied = set_bit(occupied, idx, false);
    if is_bit_set(unread, idx) {
        let unread = set_bit(unread, idx, false);
        TCU::set_ep(ep, EpReg::BUF_UNREAD, unread);
        TCU::set_ep(
            ep,
            EpReg::BUF_MSG_CNT,
            TCU::get_ep(ep, EpReg::BUF_MSG_CNT) - 1,
        );
        fetched_msg();
    }
    TCU::set_ep(ep, EpReg::BUF_OCCUPIED, occupied);

    log_tcu!("EP{}: acked message at index {}", ep, idx);

    Ok((0, TOTAL_EPS))
}

fn prepare_fetch(ep: EpId) -> Result<(TileId, EpId), Error> {
    let msgs = TCU::get_ep(ep, EpReg::BUF_MSG_CNT);
    if msgs == 0 {
        TCU::set_cmd(CmdReg::OFFSET, !0);
        return Ok((0, TOTAL_EPS));
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

        TCU::set_cmd(CmdReg::OFFSET, idx * (1 << msg_ord));

        fetched_msg();

        Ok((0, TOTAL_EPS))
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

fn received_msg() {
    MSG_CNT.set(MSG_CNT.get() + 1);
    log_tcu!("TCU: received message");
    if !matches!(SLEEP.get(), SleepState::None) {
        stop_sleep();
    }
}

fn fetched_msg() {
    MSG_CNT.set(MSG_CNT.get() - 1);
    log_tcu!("TCU: fetched message");
}

fn start_sleep() {
    let timeout = TCU::get_cmd(CmdReg::OFFSET);
    if MSG_CNT.get() == 0 {
        SLEEP.set(match timeout {
            t if t == UNLIM_TIMEOUT => SleepState::UntilMsg,
            t => SleepState::UntilTimeout(TCU::nanotime() + t),
        });
        log_tcu!("TCU: sleep started ({:?})", SLEEP.get());
    }
    else {
        // still unread messages -> no sleep. ack is sent if command is ready
        TCU::set_cmd(CmdReg::CTRL, 0);
    }
}

fn stop_sleep() {
    log_tcu!("TCU: sleep stopped (messages: {})", MSG_CNT.get());
    SLEEP.set(SleepState::None);
    // provide feedback to SW
    TCU::set_cmd(CmdReg::CTRL, 0);
    get_backend().send_ack();
}

fn handle_msg(buf: &RefMut<'_, Buffer>, ep: EpId, len: usize) {
    let msg_ord = TCU::get_ep(ep, EpReg::BUF_MSGORDER);
    let msg_size = 1 << msg_ord;
    if len > msg_size {
        log_tcu_critical!(
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
        let dst = (envdata::rbuf_start() as u64 + addr + idx * (1 << msg_ord)) as *mut u8;
        let src = &buf.header as *const Header as *const u8;
        unsafe {
            util::slice_for_mut(dst, len).copy_from_slice(util::slice_for(src, len));
        }

        received_msg();
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

    log_tcu_critical!("TCU-error: EP{}: dropping msg because no slot is free", ep);
}

fn handle_write_cmd(
    backend: &backend::SocketBackend,
    buf: &mut RefMut<'_, Buffer>,
    ep: EpId,
) -> Result<(), Error> {
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
        assert!(length as usize <= MAX_MSG_SIZE - 2 * mem::size_of::<u64>());

        unsafe {
            libc::memcpy(
                offset as *mut libc::c_void,
                data[2..].as_ptr() as *const libc::c_void,
                length as usize,
            );
        }
    }

    let dst_tile = buf.header.tile as TileId;
    let dst_ep = buf.header.rpl_ep as EpId;

    buf.header.opcode = Command::RESP.val as u8;
    buf.header.credits = 0;
    buf.header.label = 0;
    buf.header.length = 0;

    send_msg(backend, buf, ep, dst_tile, dst_ep)
}

fn handle_read_cmd(
    backend: &backend::SocketBackend,
    buf: &mut RefMut<'_, Buffer>,
    ep: EpId,
) -> Result<(), Error> {
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
    assert!(length as usize <= MAX_MSG_SIZE - 3 * mem::size_of::<u64>());

    let dst_tile = buf.header.tile as TileId;
    let dst_ep = buf.header.rpl_ep as EpId;

    buf.header.opcode = Command::RESP.val as u8;
    buf.header.credits = 0;
    buf.header.label = 0;
    buf.header.length = length as usize + 3 * mem::size_of::<u64>();

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

    send_msg(backend, buf, ep, dst_tile, dst_ep)
}

fn handle_resp_cmd(backend: &backend::SocketBackend, buf: &RefMut<'_, Buffer>) {
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
        assert!(length as usize <= MAX_MSG_SIZE - 3 * mem::size_of::<usize>());

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
    backend.send_ack();
}

#[rustfmt::skip]
fn send_msg(
    backend: &backend::SocketBackend,
    buf: &RefMut<'_, Buffer>,
    ep: EpId,
    dst_tile: TileId,
    dst_ep: EpId,
) -> Result<(), Error> {
    log_tcu!(
        "{} {:3}b lbl={:#016x} over {} to tile:ep={}:{} (crd={:#x} rep={})",
        if buf.header.opcode == Command::REPLY.val as u8 { ">>" } else { "->" },
        { buf.header.length },
        { buf.header.label },
        buf.header.snd_ep,
        dst_tile,
        dst_ep,
        TCU::get_ep(ep, EpReg::CREDITS),
        buf.header.rpl_ep
    );

    if backend.send(dst_tile, dst_ep, buf) {
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

    let res = if ep >= TOTAL_EPS {
        log_tcu!("TCU-error: invalid ep-id ({})", ep);
        Err(Error::new(Code::InvArgs))
    }
    else {
        let ctrl = TCU::get_cmd(CmdReg::CTRL);
        let op: Command = Command::from((ctrl >> 3) & 0xF);

        log_tcu!("TCU: handling command {}", op);

        let res = match op {
            Command::SEND => prepare_send(ep),
            Command::REPLY => prepare_reply(ep),
            Command::READ => prepare_read(ep),
            Command::WRITE => prepare_write(ep),
            Command::FETCH_MSG => prepare_fetch(ep),
            Command::ACK_MSG => prepare_ack(ep),
            Command::SLEEP => return start_sleep(),
            _ => Err(Error::new(Code::NotSup)),
        };

        match res {
            Ok((dst_tile, dst_ep)) if dst_ep < TOTAL_EPS => {
                let mut buf = BUFFER.borrow_mut();
                buf.header.opcode = op.val as u8;

                if op != Command::REPLY {
                    // reply cap
                    buf.header.has_replycap = 1;
                    buf.header.tile = envdata::get().tile_id as u16;
                    buf.header.snd_ep = ep as u8;
                    buf.header.rpl_ep = TCU::get_cmd(CmdReg::REPLY_EPID) as u8;
                    buf.header.reply_label = TCU::get_cmd(CmdReg::REPLY_LBL);
                }

                match send_msg(backend, &buf, ep, dst_tile, dst_ep) {
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
    let mut buf = BUFFER.borrow_mut();
    if let Some(size) = backend.receive(ep, &mut buf) {
        match Command::from(buf.header.opcode as Reg) {
            Command::SEND | Command::REPLY => handle_msg(&buf, ep, size),
            Command::READ => handle_read_cmd(backend, &mut buf, ep).unwrap(),
            Command::WRITE => handle_write_cmd(backend, &mut buf, ep).unwrap(),
            Command::RESP => handle_resp_cmd(backend, &buf),
            _ => panic!("Not supported!"),
        }

        // refill credits
        let crd_ep = buf.header.crd_ep as EpId;
        if crd_ep >= TOTAL_EPS {
            log_tcu_critical!("TCU-error: should give credits to ep {}", crd_ep);
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
            size - mem::size_of::<Header>(),
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

// TODO unfortunately, we have to use an unsafe cell here, because it's used from multiple threads
// and we don't have a standard library. we therefore manually take care that this is correct.
static BACKEND: StaticUnsafeCell<Option<backend::SocketBackend>> = StaticUnsafeCell::new(None);
static RUN: atomic::AtomicUsize = atomic::AtomicUsize::new(1);
static mut TID: libc::pthread_t = 0;

pub(crate) fn get_backend() -> &'static mut backend::SocketBackend {
    // safety: see comment for BACKEND
    unsafe { BACKEND.get_mut().as_mut().unwrap() }
}

extern "C" fn sigchild(_: i32) {
    // send notification to kernel
    unsafe {
        let mut status: i32 = 0;
        let pid = libc::wait(&mut status as *mut i32);
        if pid != -1 {
            get_backend().notify_kernel(pid, status);
        }

        libc::signal(libc::SIGCHLD, sigchild as usize);
    }
}

pub fn bind_knotify() {
    get_backend().bind_knotify();
}

pub fn receive_knotify() -> Option<(libc::pid_t, i32)> {
    get_backend().receive_knotify()
}

extern "C" fn run(_arg: *mut libc::c_void) -> *mut libc::c_void {
    let backend = get_backend();

    unsafe {
        libc::signal(libc::SIGCHLD, sigchild as usize);
    }

    while RUN.load(atomic::Ordering::Relaxed) == 1 {
        if backend.recv_command() {
            assert!((TCU::get_cmd(CmdReg::CTRL) & Control::START.bits()) != 0);
            handle_command(backend);
            // for read and write, we get a response later
            if TCU::is_ready() {
                backend.send_ack();
            }
        }

        for ep in 0..TOTAL_EPS {
            handle_receive(backend, ep);
        }

        let now = TCU::nanotime();
        if let SleepState::UntilTimeout(end) = SLEEP.get() {
            if now >= end {
                stop_sleep();
            }
        }

        let timeout = match SLEEP.get() {
            SleepState::UntilTimeout(end) => Some(end.saturating_sub(now)),
            _ => None,
        };
        if backend.wait_for_work(timeout) {
            // if an additional fd is ready and the CPU is sleeping, wake it up
            if !matches!(SLEEP.get(), SleepState::None) {
                stop_sleep();
            }
        }
    }

    // deny further receives
    backend.shutdown();

    // handle all outstanding messages
    loop {
        let mut received = false;
        for ep in 0..TOTAL_EPS {
            received |= handle_receive(backend, ep);
        }
        if !received {
            break;
        }

        backend.wait_for_work(None);
    }

    ptr::null_mut()
}

pub fn init() {
    LOG.set(io::log::Log::new());
    LOG.borrow_mut().init(envdata::get().tile_id, "TCU");

    // safety: we pass in a newly constructed SocketBackend and have not initialized BACKEND before.
    unsafe {
        BACKEND.set(Some(backend::SocketBackend::new()));
    }

    unsafe {
        let res = libc::pthread_create(&mut TID, ptr::null(), run, ptr::null_mut());
        assert!(res == 0);
    }
}

pub fn deinit() {
    RUN.store(0, atomic::Ordering::Relaxed);
    // wakeup the thread, if necessary
    get_backend().send_command();

    unsafe {
        // libc::pthread_kill(TID, libc::SIGUSR1);
        assert!(libc::pthread_join(TID, ptr::null_mut()) == 0);
    }

    // first remove the signal handler to ensure that we don't access BACKEND again
    unsafe {
        libc::signal(libc::SIGCHLD, libc::SIG_IGN);
    }

    // safety: the thread is killed, so there are no other references left
    unsafe {
        BACKEND.set(None);
    }
}
