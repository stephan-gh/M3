/*
 * Copyright (C) 2015-2020, Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Copyright (C) 2019-2020, Tendsin Mende <tendsin@protonmail.com>
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

#![feature(core_intrinsics)]
#![no_std]

#[macro_use]
extern crate m3;

mod backend;
mod buf;
mod data;
mod fs_handle;
mod ops;
mod sess;

use crate::backend::{Backend, DiskBackend, MemBackend};
use crate::fs_handle::M3FSHandle;
use crate::sess::{FSSession, M3FSSession, MetaSession};

use m3::{
    cap::Selector,
    cell::{LazyStaticCell, StaticCell},
    col::{String, ToString, Vec},
    com::GateIStream,
    env,
    errors::{Code, Error},
    goff,
    pes::VPE,
    server::{
        server_loop, CapExchange, Handler, RequestHandler, Server, SessId, SessionContainer,
        DEF_MAX_CLIENTS,
    },
    tcu::{EpId, Label, TOTAL_EPS},
    vfs::{FSOperation, GenFileOp},
};

// Sets the logging behavior
pub const LOG_DEF: bool = false;
pub const LOG_SESSION: bool = false;
pub const LOG_ALLOC: bool = false;
pub const LOG_BUFFER: bool = false;
pub const LOG_DIRS: bool = false;
pub const LOG_INODES: bool = false;
pub const LOG_LINKS: bool = false;
pub const LOG_FIND: bool = false;

// Server constants
const FS_IMG_OFFSET: goff = 0;
const MAX_CLIENTS: usize = DEF_MAX_CLIENTS;
const MSG_SIZE: usize = 128;

// The global request handler
static REQHDL: LazyStaticCell<RequestHandler> = LazyStaticCell::default();

// The global file handle in this process
static FSHANDLE: StaticCell<Option<M3FSHandle>> = StaticCell::new(None);

fn hdl() -> &'static mut M3FSHandle {
    FSHANDLE.get_mut().as_mut().unwrap()
}

int_enum! {
    pub struct M3FSOperation : u64 {
        const STAT      = GenFileOp::STAT.val;
        const SEEK      = GenFileOp::SEEK.val;
        const NEXT_IN   = GenFileOp::NEXT_IN.val;
        const NEXT_OUT  = GenFileOp::NEXT_OUT.val;
        const COMMIT    = GenFileOp::COMMIT.val;
        const SYNC      = GenFileOp::SYNC.val;
        const CLOSE     = GenFileOp::CLOSE.val;
        const FSTAT     = FSOperation::STAT.val;
        const MKDIR     = FSOperation::MKDIR.val;
        const RMDIR     = FSOperation::RMDIR.val;
        const LINK      = FSOperation::LINK.val;
        const UNLINK    = FSOperation::UNLINK.val;
        const RENAME    = FSOperation::RENAME.val;
    }
}

struct M3FSRequestHandler {
    sel: Selector,
    sessions: SessionContainer<FSSession>,
}

impl M3FSRequestHandler {
    fn new<B>(backend: B, settings: FsSettings) -> Result<Self, Error>
    where
        B: Backend + 'static,
    {
        // init thread manager, otherwise the waiting within the file and meta buffer impl. panics.
        thread::init();
        FSHANDLE.set(Some(M3FSHandle::new(backend, settings)));

        let container = SessionContainer::new(DEF_MAX_CLIENTS);

        Ok(M3FSRequestHandler {
            sel: 0, // gets set later in main
            sessions: container,
        })
    }

    pub fn handle(&mut self, op: M3FSOperation, input: &mut GateIStream) -> Result<(), Error> {
        log!(LOG_DEF, "[{}] fs::handle(op={})", input.label(), op);

        let res = match op {
            M3FSOperation::NEXT_IN => self.execute_on_session(input, |sess, is| sess.next_in(is)),
            M3FSOperation::NEXT_OUT => self.execute_on_session(input, |sess, is| sess.next_out(is)),
            M3FSOperation::COMMIT => self.execute_on_session(input, |sess, is| sess.commit(is)),
            M3FSOperation::CLOSE => {
                // get session id, then notify caller that we closed, finally close self
                let sid = input.label() as SessId;
                reply_vmsg!(input, 0).ok();
                self.close_session(sid)
            },
            M3FSOperation::STAT => self.execute_on_session(input, |sess, is| sess.stat(is)),
            M3FSOperation::SEEK => self.execute_on_session(input, |sess, is| sess.seek(is)),
            M3FSOperation::FSTAT => self.execute_on_session(input, |sess, is| sess.stat(is)),
            M3FSOperation::MKDIR => self.execute_on_session(input, |sess, is| sess.mkdir(is)),
            M3FSOperation::RMDIR => self.execute_on_session(input, |sess, is| sess.rmdir(is)),
            M3FSOperation::LINK => self.execute_on_session(input, |sess, is| sess.link(is)),
            M3FSOperation::UNLINK => self.execute_on_session(input, |sess, is| sess.unlink(is)),
            M3FSOperation::RENAME => self.execute_on_session(input, |sess, is| sess.rename(is)),
            M3FSOperation::SYNC => self.execute_on_session(input, |sess, is| sess.sync(is)),
            _ => Err(Error::new(Code::InvArgs)),
        };

        if let Err(ref e) = res {
            input.reply_error(e.code()).ok();
        }

        log!(
            LOG_DEF,
            "[{}] fs::handle(op={}) -> {:?}",
            input.label(),
            op,
            res.as_ref().map_err(|e| e.code()),
        );
        Ok(())
    }

    fn execute_on_session<F, R>(&mut self, is: &mut GateIStream, function: F) -> Result<R, Error>
    where
        F: Fn(&mut FSSession, &mut GateIStream) -> Result<R, Error>,
    {
        let session_id: SessId = is.label() as SessId;
        if let Some(sess) = self.sessions.get_mut(session_id) {
            function(sess, is)
        }
        else {
            Err(Error::new(Code::InvArgs))
        }
    }

    fn close_session(&mut self, sid: SessId) -> Result<(), Error> {
        log!(LOG_SESSION, "[{}] closing session", sid);

        {
            let session = self.remove_session(sid)?;
            match session {
                FSSession::Meta(ref meta) => {
                    // remove contained file sessions
                    for fsid in meta.file_sessions() {
                        self.remove_session(*fsid)?;

                        // see below
                        REQHDL.recv_gate().drop_msgs_with(*fsid as Label);
                    }
                },

                FSSession::File(ref file) => {
                    // remove file session from parent meta session
                    let parent_meta_session = self.sessions.get_mut(file.meta_sess()).unwrap();
                    match parent_meta_session {
                        FSSession::Meta(ref mut pms) => pms.remove_file(sid),
                        _ => panic!("FileSession's parent is not a MetaSession!?"),
                    }
                },
            }
        }

        // now that the session has been dropped and thus the SendGate revoked, drop remaining
        // messages for this session
        REQHDL.recv_gate().drop_msgs_with(sid as Label);

        Ok(())
    }

    fn remove_session(&mut self, sid: SessId) -> Result<FSSession, Error> {
        let session = self
            .sessions
            .get_mut(sid)
            .ok_or_else(|| Error::new(Code::InvArgs))?;

        let creator = session.creator();
        Ok(self.sessions.remove(creator, sid))
    }
}

impl Handler<FSSession> for M3FSRequestHandler {
    fn sessions(&mut self) -> &mut SessionContainer<FSSession> {
        &mut self.sessions
    }

    /// Creates a new session with `arg` as an argument for the service with selector `srv_sel`.
    ///
    /// Returns the session selector and the session identifier.
    fn open(
        &mut self,
        crt: usize,
        srv_sel: Selector,
        arg: &str,
    ) -> Result<(Selector, SessId), Error> {
        // get max number of files
        let mut max_files: usize = 16;
        if arg.len() > 6 && &arg[..6] == "file=" {
            max_files = arg[6..].parse().map_err(|_| Error::new(Code::InvArgs))?;
        }

        // get the id this session would belong to.
        let sessid = self.sessions.next_id()?;

        self.sessions.add_next(crt, srv_sel, true, |sess| {
            log!(
                crate::LOG_SESSION,
                "[{}] creating session(crt={})",
                sess.ident(),
                crt
            );
            Ok(FSSession::Meta(MetaSession::new(
                sess, sessid, crt, max_files,
            )))
        })
    }

    /// Let's the client obtain a capability from the server
    fn obtain(&mut self, crt: usize, sid: SessId, data: &mut CapExchange) -> Result<(), Error> {
        log!(LOG_DEF, "[{}] fs::obtain(crt={})", sid, crt);

        if !self.sessions.can_add(crt) {
            return Err(Error::new(Code::NoSpace));
        }

        // get some values now, because we cannot borrow self while holding the session reference
        let next_sess_id = self.sessions.next_id()?;
        let sel: Selector = self.sel;

        let session = self
            .sessions
            .get_mut(sid)
            .ok_or_else(|| Error::new(Code::InvArgs))?;
        match session {
            FSSession::Meta(meta) => {
                if data.in_args().size() == 0 {
                    log!(crate::LOG_DEF, "[{}] fs::get_sgate()", sid);
                    meta.get_sgate(data)
                }
                else {
                    log!(crate::LOG_DEF, "[{}] fs::open_file()", sid);
                    let file_session = meta.open_file(sel, crt, data, next_sess_id)?;

                    self.sessions
                        .add(crt, next_sess_id, FSSession::File(file_session))
                }
            },

            FSSession::File(file) => {
                if data.in_args().size() == 0 {
                    log!(crate::LOG_DEF, "[{}] fs::clone()", sid);

                    let nfile_session = file.clone(sel, crt, next_sess_id, data)?;

                    self.sessions
                        .add(crt, next_sess_id, FSSession::File(nfile_session))
                }
                else {
                    log!(crate::LOG_DEF, "[{}] fs::get_mem()", sid);
                    file.get_mem(data)
                }
            },
        }
    }

    /// Let's the client delegate a capability to the server
    fn delegate(&mut self, _crt: usize, sid: SessId, data: &mut CapExchange) -> Result<(), Error> {
        log!(LOG_DEF, "[{}] fs::delegate()", sid);

        let session = self
            .sessions
            .get_mut(sid)
            .ok_or_else(|| Error::new(Code::InvArgs))?;

        if data.in_caps() != 1 || !session.is_file_session() {
            return Err(Error::new(Code::NotSup));
        }

        if let FSSession::File(fs) = session {
            let new_sel: Selector = VPE::cur().alloc_sel();

            log!(
                LOG_DEF,
                "[{}] fs::delegate(): set_ep(sel: {})",
                sid,
                new_sel,
            );

            fs.set_ep(new_sel);
            data.out_caps(m3::kif::CapRngDesc::new(
                m3::kif::CapType::OBJECT,
                new_sel,
                1,
            ));
        }
        else {
            panic!("delegate on none FileSession, should not happen!");
        }

        Ok(())
    }

    fn close(&mut self, _crt: usize, sid: SessId) {
        self.close_session(sid).ok();
    }

    fn shutdown(&mut self) {
        crate::hdl()
            .flush_buffer()
            .expect("buffer flush at shutdown failed");
    }
}

#[derive(Clone, Debug)]
pub struct FsSettings {
    name: String,
    backend: String,
    fs_size: usize,
    extend: usize,
    max_load: usize,
    clear: bool,
    selector: Option<Selector>,
    ep: EpId,
    fs_offset: goff,
}

impl core::default::Default for FsSettings {
    fn default() -> Self {
        FsSettings {
            name: String::from("m3fs"),
            backend: String::from("mem"),
            fs_size: 0,
            extend: 128,
            max_load: 128,
            clear: false,
            selector: None,
            ep: TOTAL_EPS,
            fs_offset: FS_IMG_OFFSET,
        }
    }
}

fn usage() -> ! {
    println!(
        "Usage: {} [-n <name>] [-s <sel>] [-e <blocks>] [-c] [-b <blocks>]",
        env::args().next().unwrap()
    );
    println!("       [-o <offset>] (disk|mem <fssize>)");
    println!();
    println!("  -n: the name of the service (m3fs by default)");
    println!("  -s: don't create service, use selectors <sel>..<sel+1>");
    println!("  -e: the number of blocks to extend files when appending");
    println!("  -c: clear allocated blocks");
    println!("  -b: the maximum number of blocks loaded from the disk");
    println!("  -o: the file system offset in DRAM");
    m3::exit(1);
}

fn parse_args() -> Result<FsSettings, String> {
    let mut settings = FsSettings::default();

    let args: Vec<&str> = env::args().collect();
    let mut i = 1;
    while i < args.len() {
        match args[i] {
            "-n" => settings.name = args[i + 1].to_string(),
            "-s" => {
                if let Ok(s) = args[i + 1].parse::<Selector>() {
                    settings.selector = Some(s);
                }
            },
            "-e" => {
                settings.extend = args[i + 1]
                    .parse::<usize>()
                    .map_err(|_| String::from("Could not parse FS extend"))?;
            },
            "-b" => {
                settings.max_load = args[i + 1]
                    .parse::<usize>()
                    .map_err(|_| String::from("Could not parse max load"))?;
            },
            "-o" => {
                settings.fs_offset = args[i + 1]
                    .parse::<goff>()
                    .map_err(|_| String::from("Failed to parse FS offset"))?;
            },
            "-c" => {
                settings.clear = true;
                i -= 1; // argument has no value
            },
            _ => break,
        }
        // move forward 2 by default, since most arguments have a value
        i += 2;
    }

    settings.backend = args[i].to_string();
    match settings.backend.as_str() {
        "mem" => {
            settings.fs_size = args[i + 1]
                .parse::<usize>()
                .map_err(|_| String::from("Failed to parse fs size"))?;
        },
        "disk" => {},
        backend => return Err(format!("Unknown backend {}", backend)),
    }

    Ok(settings)
}

#[no_mangle]
pub fn main() -> i32 {
    // parse arguments
    let settings = parse_args().unwrap_or_else(|e| {
        println!("Invalid arguments: {}", e);
        usage();
    });
    log!(crate::LOG_DEF, "{:#?}", settings);

    // create backend for the file system
    let mut hdl = if settings.backend == "mem" {
        let backend = MemBackend::new(settings.fs_offset, settings.fs_size);
        M3FSRequestHandler::new(backend, settings.clone())
            .expect("Failed to create m3fs handler based on memory backend")
    }
    else {
        let backend = DiskBackend::new().expect("Failed to initialize disk backend!");
        M3FSRequestHandler::new(backend, settings.clone())
            .expect("Failed to create m3fs handler based on disk backend")
    };

    // create new server for file system and pass on selector to handler
    let serv = Server::new(&settings.name, &mut hdl).expect("Could not create service 'm3fs'");
    hdl.sel = serv.sel();

    // create request handler
    REQHDL.set(
        RequestHandler::new_with(MAX_CLIENTS, MSG_SIZE).expect("Unable to create request handler"),
    );

    server_loop(|| {
        // handle message that is given to the server
        serv.handle_ctrl_chan(&mut hdl)?;
        REQHDL
            .get_mut()
            .handle(|op, mut is| hdl.handle(op, &mut is))
    })
    .ok();

    0
}
