/*
 * Copyright (C) 2021 Nils Asmussen, Barkhausen Institut
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
use m3::cell::{LazyStaticCell, LazyStaticRefCell};
use m3::cfg;
use m3::col::Vec;
use m3::com::MemGate;
use m3::env;
use m3::errors::{Code, Error};
use m3::format;
use m3::goff;
use m3::int_enum;
use m3::io::Read;
use m3::kif::{self, Perm};
use m3::log;
use m3::println;
use m3::server::{
    server_loop, CapExchange, Handler, Server, SessId, SessionContainer, DEF_MAX_CLIENTS,
};
use m3::session::ServerSession;
use m3::tiles::OwnActivity;
use m3::util::math;
use m3::vfs::OpenFlags;
use m3::vfs::VFS;

pub const LOG_DEF: bool = false;

static AUDIO_DATA: LazyStaticRefCell<MemGate> = LazyStaticRefCell::default();
static AUDIO_SIZE: LazyStaticCell<usize> = LazyStaticCell::default();

int_enum! {
    struct ImgSndOp : u64 {
        const RECV       = 0;
    }
}

#[derive(Debug)]
struct MicSession {
    crt: usize,
    _sess: ServerSession,
    img: Option<MemGate>,
}

struct MicHandler {
    sessions: SessionContainer<MicSession>,
}

impl MicHandler {
    fn new_sess(crt: usize, sess: ServerSession) -> MicSession {
        log!(crate::LOG_DEF, "[{}] vamic::new()", sess.ident());
        MicSession {
            crt,
            _sess: sess,
            img: None,
        }
    }

    fn close_sess(&mut self, sid: SessId) -> Result<(), Error> {
        log!(crate::LOG_DEF, "[{}] vamic::close()", sid);
        let crt = self.sessions.get(sid).unwrap().crt;
        self.sessions.remove(crt, sid);
        Ok(())
    }
}

impl Handler<MicSession> for MicHandler {
    fn sessions(&mut self) -> &mut m3::server::SessionContainer<MicSession> {
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

    fn obtain(
        &mut self,
        _crt: usize,
        sid: SessId,
        xchg: &mut CapExchange<'_>,
    ) -> Result<(), Error> {
        log!(crate::LOG_DEF, "[{}] vamic::recv()", sid);

        if xchg.in_caps() != 1 {
            return Err(Error::new(Code::InvArgs));
        }

        let op = xchg.in_args().pop::<ImgSndOp>()?;
        if op != ImgSndOp::RECV {
            return Err(Error::new(Code::InvArgs));
        }

        let sess = self.sessions.get_mut(sid).unwrap();

        // derive a read-only memory cap for the client. this revokes the previous memory cap, if
        // there was any.
        sess.img = Some(AUDIO_DATA.borrow().derive(0, AUDIO_SIZE.get(), Perm::R)?);
        xchg.out_args().push(AUDIO_SIZE.get());
        xchg.out_caps(kif::CapRngDesc::new(
            kif::CapType::OBJECT,
            sess.img.as_ref().unwrap().sel(),
            1,
        ));

        Ok(())
    }

    fn close(&mut self, _crt: usize, sid: SessId) {
        self.close_sess(sid).ok();
    }
}

fn usage(name: &str) -> ! {
    println!("Usage: {} <file>", name);
    OwnActivity::exit(Err(Error::new(Code::InvArgs)));
}

#[no_mangle]
pub fn main() -> Result<(), Error> {
    let args = env::args().collect::<Vec<&str>>();
    if args.len() != 2 {
        usage(args[0]);
    }

    // actually, we should read the audio samples from an actual microphone in real time.
    // since we don't have this option and for benchmarking simplicity, we pretend that audio data
    // is always available. to use real audio data, we read a wav file from the FS, read it into
    // memory and provide our clients read-only access.

    // get file size
    let info =
        VFS::stat(args[1]).unwrap_or_else(|_| panic!("{}", format!("unable to stat {}", args[1])));
    let buf_size = math::round_up(info.size, cfg::PAGE_SIZE);

    // create buffer
    AUDIO_SIZE.set(info.size);
    AUDIO_DATA.set(MemGate::new(buf_size, Perm::RW).expect("unable to allocate buffer"));

    // read file into buffer
    let mut local = [0u8; 1024];
    let mut file = VFS::open(args[1], OpenFlags::R)
        .unwrap_or_else(|_| panic!("{}", format!("unable to open {} for reading", args[1])));
    let mut off = 0;
    loop {
        let amount = file.read(&mut local).expect("read failed");
        if amount == 0 {
            break;
        }
        AUDIO_DATA
            .borrow_mut()
            .write(&local[..amount], off)
            .expect("write failed");
        off += amount as goff;
    }

    let mut hdl = MicHandler {
        sessions: SessionContainer::new(DEF_MAX_CLIENTS),
    };

    let srv = Server::new("vamic", &mut hdl).expect("Unable to create service 'vamic'");

    server_loop(|| srv.handle_ctrl_chan(&mut hdl)).ok();

    Ok(())
}
