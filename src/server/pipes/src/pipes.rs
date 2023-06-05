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

mod chan;
mod meta;
mod pipe;
mod sess;

use m3::col::{String, Vec};
use m3::com::opcodes;
use m3::env;
use m3::errors::{Code, Error};
use m3::println;
use m3::server::{ExcType, RequestHandler, Server, DEF_MAX_CLIENTS, DEF_MSG_SIZE};
use m3::tiles::OwnActivity;

use sess::PipesSession;

#[derive(Clone, Debug)]
pub struct PipesSettings {
    max_clients: usize,
}

impl Default for PipesSettings {
    fn default() -> Self {
        PipesSettings {
            max_clients: DEF_MAX_CLIENTS,
        }
    }
}

fn usage() -> ! {
    println!("Usage: {} [-m <clients>]", env::args().next().unwrap());
    println!();
    println!("  -m: the maximum number of clients (receive slots)");
    OwnActivity::exit_with(Code::InvArgs);
}

fn parse_args() -> Result<PipesSettings, String> {
    let mut settings = PipesSettings::default();

    let args: Vec<&str> = env::args().collect();
    let mut i = 1;
    while i < args.len() {
        match args[i] {
            "-m" => {
                settings.max_clients = args[i + 1]
                    .parse::<usize>()
                    .map_err(|_| String::from("Failed to parse client count"))?;
                i += 1;
            },
            _ => break,
        }
        i += 1;
    }
    Ok(settings)
}

#[no_mangle]
pub fn main() -> Result<(), Error> {
    let settings = parse_args().unwrap_or_else(|e| {
        println!("Invalid arguments: {}", e);
        usage();
    });

    // create request handler and server
    let mut hdl = RequestHandler::new_with(settings.max_clients, DEF_MSG_SIZE, 1)
        .expect("Unable to create request handler");
    let mut srv = Server::new("pipes", &mut hdl).expect("Unable to create service 'pipes'");

    // register capability handler
    use opcodes::Pipe;
    hdl.reg_cap_handler(Pipe::OpenPipe, ExcType::Obt(1), PipesSession::open_pipe);
    hdl.reg_cap_handler(Pipe::OpenChan, ExcType::Obt(2), PipesSession::open_chan);
    hdl.reg_cap_handler(Pipe::SetMem, ExcType::Del(1), PipesSession::set_mem);
    hdl.reg_cap_handler(Pipe::CloneFile, ExcType::Obt(2), PipesSession::clone);
    hdl.reg_cap_handler(Pipe::SetDest, ExcType::Del(1), PipesSession::set_dest);
    hdl.reg_cap_handler(
        Pipe::EnableNotify,
        ExcType::Del(1),
        PipesSession::enable_notify,
    );

    // register message handler
    hdl.reg_msg_handler(Pipe::NextIn, |sess, is| {
        sess.with_chan(is, |c, is| c.next_in(is))
    });
    hdl.reg_msg_handler(Pipe::NextOut, |sess, is| {
        sess.with_chan(is, |c, is| c.next_out(is))
    });
    hdl.reg_msg_handler(Pipe::Commit, |sess, is| {
        sess.with_chan(is, |c, is| c.commit(is))
    });
    hdl.reg_msg_handler(Pipe::ReqNotify, |sess, is| {
        sess.with_chan(is, |c, is| c.request_notify(is))
    });
    hdl.reg_msg_handler(Pipe::FStat, |sess, is| {
        sess.with_chan(is, |c, is| c.stat(is))
    });
    hdl.reg_msg_handler(Pipe::Seek, |_sess, _is| Err(Error::new(Code::SeekPipe)));

    hdl.run(&mut srv).expect("Server loop failed");

    Ok(())
}
