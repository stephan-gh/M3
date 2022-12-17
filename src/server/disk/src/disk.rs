/*
 * Copyright (C) 2020-2022 Nils Asmussen, Barkhausen Institut
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

mod backend;
mod gem5;
mod partition;

use m3::cap::Selector;
use m3::cell::{LazyReadOnlyCell, LazyStaticRefCell};
use m3::col::Treap;
use m3::col::Vec;
use m3::com::{GateIStream, MemGate, SGateArgs, SendGate};
use m3::env;
use m3::errors::{Code, Error};
use m3::kif;
use m3::log;
use m3::server::{
    server_loop, CapExchange, Handler, RequestHandler, Server, SessId, SessionContainer,
    DEF_MAX_CLIENTS,
};
use m3::session::{BlockNo, BlockRange, DiskOperation, ServerSession};
use m3::tcu::Label;
use m3::tiles::Activity;

use backend::BlockDevice;
use gem5::IDEBlockDevice;

pub const LOG_DEF: bool = false;
pub const LOG_ALL: bool = false;

// we can only read 255 sectors (<31 blocks) at once (see ata.cc ata_setupCommand)
// and the max DMA size is 0x10000 in gem5
const MAX_DMA_SIZE: usize = 0x10000;

const MIN_SEC_SIZE: usize = 512;

static REQHDL: LazyReadOnlyCell<RequestHandler> = LazyReadOnlyCell::default();
static DEVICE: LazyStaticRefCell<IDEBlockDevice> = LazyStaticRefCell::default();

struct DiskSession {
    sess: ServerSession,
    part: usize,
    sgates: Vec<SendGate>,
    blocks: Treap<BlockRange, Selector>,
}

impl DiskSession {
    fn read(&mut self, is: &mut GateIStream<'_>) -> Result<(), Error> {
        self.read_write(is, "read", |part, mgate, off, start, count| {
            DEVICE.borrow_mut().read(part, mgate, off, start, count)
        })
    }

    fn write(&mut self, is: &mut GateIStream<'_>) -> Result<(), Error> {
        self.read_write(is, "write", |part, mgate, off, start, count| {
            DEVICE.borrow_mut().write(part, mgate, off, start, count)
        })
    }

    fn read_write<F>(&mut self, is: &mut GateIStream<'_>, name: &str, func: F) -> Result<(), Error>
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

        let range = BlockRange::new_range(cap, 1);
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

        is.reply_error(Code::Success)
    }
}

struct DiskHandler {
    sessions: SessionContainer<DiskSession>,
}

impl Handler<DiskSession> for DiskHandler {
    fn sessions(&mut self) -> &mut m3::server::SessionContainer<DiskSession> {
        &mut self.sessions
    }

    fn open(
        &mut self,
        crt: usize,
        srv_sel: Selector,
        arg: &str,
    ) -> Result<(Selector, SessId), Error> {
        let dev = arg
            .parse::<usize>()
            .map_err(|_| Error::new(Code::InvArgs))?;
        if !DEVICE.borrow().partition_exists(dev) {
            return Err(Error::new(Code::InvArgs));
        }

        self.sessions.add_next(crt, srv_sel, false, |sess| {
            log!(crate::LOG_DEF, "[{}] disk::open(dev={})", sess.ident(), dev);
            Ok(DiskSession {
                sess,
                part: dev,
                sgates: Vec::new(),
                blocks: Treap::new(),
            })
        })
    }

    fn obtain(
        &mut self,
        _crt: usize,
        sid: SessId,
        xchg: &mut CapExchange<'_>,
    ) -> Result<(), Error> {
        if xchg.in_caps() != 1 {
            return Err(Error::new(Code::InvArgs));
        }

        log!(crate::LOG_DEF, "[{}] disk::get_sgate()", sid);

        let sess = self.sessions.get_mut(sid).unwrap();
        let sgate = SendGate::new_with(
            SGateArgs::new(REQHDL.get().recv_gate())
                .label(sid as Label)
                .credits(1),
        )?;
        let sel = sgate.sel();
        sess.sgates.push(sgate);

        xchg.out_caps(kif::CapRngDesc::new(kif::CapType::OBJECT, sel, 1));
        Ok(())
    }

    fn delegate(
        &mut self,
        _crt: usize,
        sid: SessId,
        xchg: &mut CapExchange<'_>,
    ) -> Result<(), Error> {
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
        let sel = Activity::own().alloc_sel();
        let range = BlockRange::new_range(bno, len);
        sess.blocks.remove(&range);
        sess.blocks.insert(range, sel);

        xchg.out_caps(kif::CapRngDesc::new(kif::CapType::OBJECT, sel, 1));
        Ok(())
    }

    fn close(&mut self, crt: usize, sid: SessId) {
        log!(crate::LOG_DEF, "[{}] disk::close()", sid);
        self.sessions.remove(crt, sid);
    }
}

#[no_mangle]
pub fn main() -> Result<(), Error> {
    let mut hdl = DiskHandler {
        sessions: SessionContainer::new(DEF_MAX_CLIENTS),
    };
    let s = Server::new("disk", &mut hdl).expect("Unable to create service 'disk'");

    DEVICE.set(IDEBlockDevice::new(env::args().collect()).expect("Unable to create block device"));
    REQHDL.set(
        RequestHandler::new_with(DEF_MAX_CLIENTS, 256).expect("Unable to create request handler"),
    );

    server_loop(|| {
        s.handle_ctrl_chan(&mut hdl)?;

        REQHDL.get().handle(|op, is| {
            let sess = hdl
                .sessions
                .get_mut(is.label() as usize)
                .ok_or_else(|| Error::new(Code::InvArgs))?;

            match op {
                DiskOperation::READ => sess.read(is),
                DiskOperation::WRITE => sess.write(is),
                _ => Err(Error::new(Code::InvArgs)),
            }
        })
    })
    .ok();

    // delete device
    DEVICE.unset();

    Ok(())
}
