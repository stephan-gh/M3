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

#[cfg(target_os = "none")]
extern crate pci;

#[cfg(target_os = "none")]
#[macro_use]
extern crate bitflags;

mod backend;
mod partition;

use core::cmp;
use m3::cap::Selector;
use m3::cell::StaticCell;
use m3::col::Treap;
use m3::col::Vec;
use m3::com::{GateIStream, MemGate, RecvGate, SGateArgs, SendGate};
use m3::env;
use m3::errors::{Code, Error};
use m3::kif;
use m3::math;
use m3::pes::VPE;
use m3::server::{server_loop, CapExchange, Handler, Server, SessId, SessionContainer};
use m3::session::ServerSession;
use m3::tcu::Label;

use backend::BlockDevice;
use backend::BlockDeviceTrait;

type BlockNo = u32;

int_enum! {
    pub struct Operation : u32 {
        const READ  = 0x0;
        const WRITE = 0x1;
    }
}

pub const LOG_DEF: bool = false;
pub const LOG_ALL: bool = false;

// we can only read 255 sectors (<31 blocks) at once (see ata.cc ata_setupCommand)
// and the max DMA size is 0x10000 in gem5
const MAX_DMA_SIZE: usize = 0x10000;

const MIN_SEC_SIZE: usize = 512;

const MSG_SIZE: usize = 256;
const MAX_CLIENTS: usize = 32;

static RGATE: StaticCell<Option<RecvGate>> = StaticCell::new(None);
static DEVICE: StaticCell<Option<BlockDevice>> = StaticCell::new(None);

fn device() -> &'static mut BlockDevice {
    DEVICE.get_mut().as_mut().unwrap()
}

fn rgate() -> &'static RecvGate {
    RGATE.get().as_ref().unwrap()
}

#[derive(Copy, Clone, Eq, PartialEq, PartialOrd)]
struct BlockRange {
    start: BlockNo,
    count: u32,
}

impl BlockRange {
    fn new(start: BlockNo, count: u32) -> Self {
        Self { start, count }
    }
}

impl cmp::Ord for BlockRange {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        if self.start >= other.start && self.start < other.start + other.count as u32 {
            cmp::Ordering::Equal
        }
        else if self.start < other.start {
            cmp::Ordering::Less
        }
        else {
            cmp::Ordering::Greater
        }
    }
}

struct DiskSession {
    sess: ServerSession,
    part: usize,
    sgates: Vec<SendGate>,
    blocks: Treap<BlockRange, Selector>,
}

impl DiskSession {
    fn read(&mut self, is: &mut GateIStream) -> Result<(), Error> {
        self.read_write(is, "read", |part, mgate, off, start, count| {
            device().read(part, &mgate, off, start, count)
        })
    }

    fn write(&mut self, is: &mut GateIStream) -> Result<(), Error> {
        self.read_write(is, "write", |part, mgate, off, start, count| {
            device().write(part, &mgate, off, start, count)
        })
    }

    fn read_write<F>(&mut self, is: &mut GateIStream, name: &str, func: F) -> Result<(), Error>
    where
        F: Fn(usize, &MemGate, usize, usize, usize) -> Result<(), Error>,
    {
        let cap: BlockNo = is.pop()?;
        let start: BlockNo = is.pop()?;
        let len: usize = is.pop()?;
        let block_size: usize = is.pop()?;
        let mut off: usize = is.pop()?;

        log!(
            crate::LOG_DEF,
            "[{}] disk::{}(cap={}, start={}, len={}, block_size={}, off={})",
            self.sess.ident(),
            name,
            cap,
            start,
            len,
            block_size,
            off
        );

        if (block_size % MIN_SEC_SIZE) != 0 {
            return Err(Error::new(Code::InvArgs));
        }

        let range = BlockRange::new(cap, 1);
        let mem_sel = self
            .blocks
            .get(&range)
            .ok_or_else(|| Error::new(Code::NoPerm))?;
        let mgate = MemGate::new_bind(*mem_sel);

        let mut start = start as usize * block_size;
        let mut len = len * block_size;

        while len >= MAX_DMA_SIZE {
            func(self.part, &mgate, off, start, MAX_DMA_SIZE)?;
            start += MAX_DMA_SIZE;
            off += MAX_DMA_SIZE;
            len -= MAX_DMA_SIZE;
        }

        // now write the rest
        if len > 0 {
            func(self.part, &mgate, off, start, len)?;
        }

        Ok(())
    }
}

struct DiskHandler {
    sessions: SessionContainer<DiskSession>,
}

impl Handler for DiskHandler {
    fn open(&mut self, srv_sel: Selector, arg: &str) -> Result<(Selector, SessId), Error> {
        let dev = arg
            .parse::<usize>()
            .map_err(|_| Error::new(Code::InvArgs))?;
        if !device().partition_exists(dev) {
            return Err(Error::new(Code::InvArgs));
        }

        self.sessions.add_next(srv_sel, false, |sess| {
            log!(crate::LOG_DEF, "[{}] disk::open(dev={})", sess.ident(), dev);
            Ok(DiskSession {
                sess,
                part: dev,
                sgates: Vec::new(),
                blocks: Treap::new(),
            })
        })
    }

    fn obtain(&mut self, sid: SessId, xchg: &mut CapExchange) -> Result<(), Error> {
        if xchg.in_caps() != 1 {
            return Err(Error::new(Code::InvArgs));
        }

        log!(crate::LOG_DEF, "[{}] disk::get_sgate()", sid);

        let sess = self.sessions.get_mut(sid).unwrap();
        let sgate = SendGate::new_with(SGateArgs::new(rgate()).label(sid as Label).credits(1))?;
        let sel = sgate.sel();
        sess.sgates.push(sgate);

        xchg.out_caps(kif::CapRngDesc::new(kif::CapType::OBJECT, sel, 1));
        Ok(())
    }

    fn delegate(&mut self, sid: SessId, xchg: &mut CapExchange) -> Result<(), Error> {
        if xchg.in_caps() != 1 {
            return Err(Error::new(Code::InvArgs));
        }

        let bno: BlockNo = xchg.in_args().pop()?;
        let len: u32 = xchg.in_args().pop()?;

        log!(
            crate::LOG_DEF,
            "[{}] disk::add_mem(bno={}, len={})",
            sid,
            bno,
            len
        );

        let sess = self.sessions.get_mut(sid).unwrap();
        let sel = VPE::cur().alloc_sel();
        let range = BlockRange::new(bno, len);
        sess.blocks.remove(&range);
        sess.blocks.insert(range, sel);

        xchg.out_caps(kif::CapRngDesc::new(kif::CapType::OBJECT, sel, 1));
        Ok(())
    }

    fn close(&mut self, sid: SessId) {
        log!(crate::LOG_DEF, "[{}] disk::close()", sid);
        self.sessions.remove(sid);
    }
}

impl DiskHandler {
    fn new() -> Self {
        Self {
            sessions: SessionContainer::new(MAX_CLIENTS),
        }
    }

    fn handle(&mut self, is: &mut GateIStream) -> Result<(), Error> {
        let sess = self
            .sessions
            .get_mut(is.label() as usize)
            .ok_or_else(|| Error::new(Code::InvArgs))?;

        let res = match is.pop::<Operation>()? {
            Operation::READ => sess.read(is),
            Operation::WRITE => sess.write(is),
            _ => Err(Error::new(Code::InvArgs)),
        };

        if let Err(e) = res {
            is.reply_error(e.code()).ok();
        }
        else {
            reply_vmsg!(is, 0).ok();
        }
        Ok(())
    }
}

#[no_mangle]
pub fn main() -> i32 {
    let s = Server::new("disk").expect("Unable to create service 'disk'");
    let mut hdl = DiskHandler::new();

    let device = BlockDevice::new(env::args().collect()).expect("Unable to create block device");
    DEVICE.set(Some(device));

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

    // delete device
    DEVICE.set(None);

    0
}
