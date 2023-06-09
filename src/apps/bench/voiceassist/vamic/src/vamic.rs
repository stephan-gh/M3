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

use m3::cell::{LazyStaticCell, LazyStaticRefCell};
use m3::cfg;
use m3::col::Vec;
use m3::com::MemGate;
use m3::env;
use m3::errors::{Code, Error};
use m3::format;
use m3::io::{LogFlags, Read};
use m3::kif::{self, Perm};
use m3::log;
use m3::mem::GlobOff;
use m3::println;
use m3::server::{
    CapExchange, ClientManager, ExcType, RequestHandler, RequestSession, Server, ServerSession,
    SessId,
};
use m3::tiles::OwnActivity;
use m3::util::math;
use m3::vfs::OpenFlags;
use m3::vfs::VFS;

static AUDIO_DATA: LazyStaticRefCell<MemGate> = LazyStaticRefCell::default();
static AUDIO_SIZE: LazyStaticCell<usize> = LazyStaticCell::default();

#[derive(Debug)]
struct MicSession {
    _serv: ServerSession,
    img: Option<MemGate>,
}

impl RequestSession for MicSession {
    fn new(serv: ServerSession, _arg: &str) -> Result<Self, Error>
    where
        Self: Sized,
    {
        log!(LogFlags::Debug, "[{}] vamic::new()", serv.id());
        Ok(MicSession {
            _serv: serv,
            img: None,
        })
    }

    fn close(&mut self, _cli: &mut ClientManager<Self>, sid: SessId, _sub_ids: &mut Vec<SessId>)
    where
        Self: Sized,
    {
        log!(LogFlags::Debug, "[{}] vamic::close()", sid);
    }
}

impl MicSession {
    fn recv(
        cli: &mut ClientManager<Self>,
        _crt: usize,
        sid: SessId,
        xchg: &mut CapExchange<'_>,
    ) -> Result<(), Error> {
        log!(LogFlags::Debug, "[{}] vamic::recv()", sid);

        let sess = cli.get_mut(sid).unwrap();

        // derive a read-only memory cap for the client. this revokes the previous memory cap, if
        // there was any.
        sess.img = Some(AUDIO_DATA.borrow().derive(0, AUDIO_SIZE.get(), Perm::R)?);

        xchg.out_args().push(AUDIO_SIZE.get());
        xchg.out_caps(kif::CapRngDesc::new(
            kif::CapType::Object,
            sess.img.as_ref().unwrap().sel(),
            1,
        ));

        Ok(())
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
        off += amount as GlobOff;
    }

    let mut hdl = RequestHandler::new().expect("Unable to create request handler");
    let mut srv = Server::new("vamic", &mut hdl).expect("Unable to create service 'vamic'");

    hdl.reg_cap_handler(0usize, ExcType::Obt(1), MicSession::recv);

    hdl.run(&mut srv).expect("Server loop failed");

    Ok(())
}
