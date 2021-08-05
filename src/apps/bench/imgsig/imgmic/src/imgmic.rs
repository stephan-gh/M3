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

use core::slice;

use m3::cap::Selector;
use m3::cell::LazyStaticCell;
use m3::cfg;
use m3::col::{String, ToString, Vec};
use m3::com::MemGate;
use m3::env;
use m3::errors::{Code, Error};
use m3::format;
use m3::int_enum;
use m3::io::Read;
use m3::kif::{self, Perm};
use m3::log;
use m3::math;
use m3::pes::VPE;
use m3::println;
use m3::server::{
    server_loop, CapExchange, Handler, Server, SessId, SessionContainer, DEF_MAX_CLIENTS,
};
use m3::session::ServerSession;
use m3::vfs::OpenFlags;
use m3::vfs::VFS;

pub const LOG_DEF: bool = true;

static FILE: LazyStaticCell<String> = LazyStaticCell::default();

int_enum! {
    struct ImgSndOp : u64 {
        const RECV       = 0;
    }
}

#[derive(Debug)]
struct SigSession {
    crt: usize,
    sess: ServerSession,
    img: Option<MemGate>,
}

struct SigHandler {
    sessions: SessionContainer<SigSession>,
}

impl SigHandler {
    fn new_sess(crt: usize, sess: ServerSession) -> SigSession {
        log!(crate::LOG_DEF, "[{}] imgsnd::new()", sess.ident());
        SigSession {
            crt,
            sess,
            img: None,
        }
    }

    fn close_sess(&mut self, sid: SessId) -> Result<(), Error> {
        log!(crate::LOG_DEF, "[{}] imgsnd::close()", sid);
        let crt = self.sessions.get(sid).unwrap().crt;
        self.sessions.remove(crt, sid);
        Ok(())
    }
}

impl Handler<SigSession> for SigHandler {
    fn sessions(&mut self) -> &mut m3::server::SessionContainer<SigSession> {
        &mut self.sessions
    }

    fn open(
        &mut self,
        crt: usize,
        srv_sel: Selector,
        _arg: &str,
    ) -> Result<(Selector, SessId), Error> {
        self.sessions
            .add_next(crt, srv_sel, false, |sess| Ok(Self::new_sess(crt, sess)))
    }

    fn obtain(&mut self, _crt: usize, sid: SessId, xchg: &mut CapExchange) -> Result<(), Error> {
        log!(crate::LOG_DEF, "[{}] imgsnd::get_sgate()", sid);

        if xchg.in_caps() != 1 {
            return Err(Error::new(Code::InvArgs));
        }

        let op = xchg.in_args().pop::<ImgSndOp>()?;
        if op != ImgSndOp::RECV {
            return Err(Error::new(Code::InvArgs));
        }

        let sess = self.sessions.get_mut(sid).unwrap();

        // revoke old MemGate and mappings, if any
        sess.img = None;

        // get file size
        let info = VFS::stat(&FILE).expect(&format!("unable to stat {}", *FILE));
        let buf_size = math::round_up(info.size, cfg::PAGE_SIZE);

        // create buffer
        let buffer = MemGate::new(buf_size, Perm::RW).expect("unable to allocate buffer");
        let virt = VPE::cur()
            .pager()
            .unwrap()
            .map_mem(0x20000000, &buffer, buf_size, Perm::RW)
            .expect("unable to map buffer");

        // read file into buffer
        let buf_slice = unsafe { slice::from_raw_parts_mut(virt as *mut u8, info.size) };
        {
            let mut file = VFS::open(&FILE, OpenFlags::R)
                .expect(&format!("unable to open {} for reading", *FILE));
            file.read_exact(buf_slice).expect("unable to read file");
        }

        // pass buffer to client
        xchg.out_caps(kif::CapRngDesc::new(
            kif::CapType::OBJECT,
            sess.img.as_ref().unwrap().sel(),
            1,
        ));
        sess.img = Some(buffer);

        Ok(())
    }

    fn close(&mut self, _crt: usize, sid: SessId) {
        self.close_sess(sid).ok();
    }
}

fn usage(name: &str) -> ! {
    println!("Usage: {} <file>", name);
    m3::exit(1);
}

#[no_mangle]
pub fn main() -> i32 {
    let args = env::args().collect::<Vec<&str>>();
    if args.len() != 2 {
        usage(args[0]);
    }

    FILE.set(args[1].to_string());


    let mut hdl = SigHandler {
        sessions: SessionContainer::new(DEF_MAX_CLIENTS),
    };

    let srv = Server::new("imgsnd", &mut hdl).expect("Unable to create service 'imgsnd'");

    server_loop(|| srv.handle_ctrl_chan(&mut hdl)).ok();

    0
}
