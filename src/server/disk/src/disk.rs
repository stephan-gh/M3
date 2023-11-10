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

use m3::cap::{SelSpace, Selector};
use m3::cell::LazyStaticRefCell;
use m3::client::{DiskBlockNo, DiskBlockRange};
use m3::col::{Treap, Vec};
use m3::com::{opcodes, GateIStream, MemGate};
use m3::env;
use m3::errors::{Code, Error};
use m3::io::LogFlags;
use m3::kif;
use m3::log;
use m3::server::{
    CapExchange, ClientManager, ExcType, RequestHandler, RequestSession, Server, ServerSession,
    SessId, DEF_MAX_CLIENTS,
};

use backend::BlockDevice;
use gem5::IDEBlockDevice;

// we can only read 255 sectors (<31 blocks) at once (see ata.cc ata_setupCommand)
// and the max DMA size is 0x10000 in gem5
const MAX_DMA_SIZE: usize = 0x10000;

const MIN_SEC_SIZE: usize = 512;

static DEVICE: LazyStaticRefCell<IDEBlockDevice> = LazyStaticRefCell::default();

struct DiskSession {
    serv: ServerSession,
    part: usize,
    blocks: Treap<DiskBlockRange, Selector>,
}

impl RequestSession for DiskSession {
    fn new(serv: ServerSession, arg: &str) -> Result<Self, Error>
    where
        Self: Sized,
    {
        let dev = arg
            .parse::<usize>()
            .map_err(|_| Error::new(Code::InvArgs))?;
        if !DEVICE.borrow().partition_exists(dev) {
            return Err(Error::new(Code::InvArgs));
        }

        log!(
            LogFlags::DiskReqs,
            "[{}] disk::open(dev={})",
            serv.id(),
            dev
        );

        Ok(DiskSession {
            serv,
            part: dev,
            blocks: Treap::new(),
        })
    }

    fn close(&mut self, _hdl: &mut ClientManager<Self>, sid: SessId, _sub_ids: &mut Vec<SessId>) {
        log!(LogFlags::DiskReqs, "[{}] disk::close()", sid);
    }
}

impl DiskSession {
    fn add_mem(
        cli: &mut ClientManager<Self>,
        _crt: usize,
        sid: SessId,
        xchg: &mut CapExchange<'_>,
    ) -> Result<(), Error> {
        let bno: DiskBlockNo = xchg.in_args().pop()?;
        let len: u32 = xchg.in_args().pop()?;

        log!(
            LogFlags::DiskReqs,
            "[{}] disk::add_mem(bno={}, len={})",
            sid,
            bno,
            len
        );

        let sess = cli.get_mut(sid).unwrap();
        let sel = SelSpace::get().alloc_sel();
        let range = DiskBlockRange::new_range(bno, len);
        sess.blocks.remove(&range);
        sess.blocks.insert(range, sel);

        xchg.out_caps(kif::CapRngDesc::new(kif::CapType::Object, sel, 1));
        Ok(())
    }

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
        let cap: DiskBlockNo = is.pop()?;
        let start: DiskBlockNo = is.pop()?;
        let len: usize = is.pop()?;
        let block_size: usize = is.pop()?;
        let mut off: usize = is.pop()?;

        log!(
            LogFlags::DiskReqs,
            "[{}] disk::{}(cap={}, start={}, len={}, block_size={}, off={})",
            self.serv.id(),
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

        let range = DiskBlockRange::new_range(cap, 1);
        let mem_sel = self
            .blocks
            .get(&range)
            .ok_or_else(|| Error::new(Code::NoPerm))?;
        let mgate = MemGate::new_bind(*mem_sel)?;

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

#[no_mangle]
pub fn main() -> Result<(), Error> {
    DEVICE.set(IDEBlockDevice::new(env::args().collect()).expect("Unable to create block device"));

    let mut hdl = RequestHandler::new_with(DEF_MAX_CLIENTS, 256, 1)
        .expect("Unable to create request handler");
    let mut srv = Server::new("disk", &mut hdl).expect("Unable to create service 'disk'");

    use opcodes::Disk;
    hdl.reg_cap_handler(Disk::AddMem, ExcType::Del(1), DiskSession::add_mem);
    hdl.reg_msg_handler(Disk::Read, DiskSession::read);
    hdl.reg_msg_handler(Disk::Write, DiskSession::write);

    hdl.run(&mut srv).expect("Server loop failed");

    // delete device
    DEVICE.unset();

    Ok(())
}
