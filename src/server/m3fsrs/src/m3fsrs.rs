#![feature(core_intrinsics)]
#![no_std]

#[macro_use]
extern crate m3;

mod backend;
mod buffer;
mod data;
mod file_buffer;
mod fs_handle;
mod internal;
mod meta_buffer;
mod sess;
mod util;

use crate::backend::{Backend, DiskBackend, MemBackend};
use crate::fs_handle::M3FSHandle;
use crate::internal::{BlockNo, Extent, SuperBlock};
use crate::sess::{FSSession, FileSession, M3FSSession, MetaSession};

use m3::{
    cap::Selector,
    cell::{LazyStaticCell, RefCell, StaticCell},
    col::{String, ToString, Vec},
    com::GateIStream,
    env,
    errors::{Code, Error},
    goff,
    pes::VPE,
    rc::Rc,
    serialize::Source,
    server::{server_loop, CapExchange, Handler, RequestHandler, Server, SessId, SessionContainer},
    tcu::{EpId, Label, EP_COUNT},
    vfs::{FSOperation, FileInfo, GenFileOp},
};

// Sets the logging behavior
pub const LOG_DEF: bool = false;
pub const LOG_SESSION: bool = false;
pub const LOG_ALLOC: bool = false;
pub const LOG_BUFFER: bool = false;
pub const LOG_DIRS: bool = false;
pub const LOG_INODES: bool = false;
pub const LOG_LINKS: bool = false;

// enables a hack that is needed when running the shell.. for some reason
const SHELL_HACK: bool = false;

// Server consts
const FS_IMG_OFFSET: goff = 0;
const MAX_CLIENTS: usize = 32;
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
    }
}

struct M3FSRequestHandler {
    sel: Selector,
    sessions: SessionContainer<FSSession>,
    in_memory: bool,
}

impl M3FSRequestHandler {
    fn new<B>(backend: B, settings: FsSettings) -> Result<Self, Error>
    where
        B: Backend + 'static,
    {
        let in_memory = backend.in_memory();
        // Init thread manager, otherwise the waiting within the file and meta buffer impl. panics.
        thread::init();
        FSHANDLE.set(Some(M3FSHandle::new(backend, settings.clone())));

        let container = SessionContainer::new(m3::server::DEF_MAX_CLIENTS);

        Ok(M3FSRequestHandler {
            sel: 0, // Gets set later in main
            sessions: container,
            in_memory,
        })
    }

    pub fn handle(&mut self, op: M3FSOperation, input: &mut GateIStream) -> Result<(), Error> {
        log!(LOG_DEF, "[{}] fs::handle(op={})", input.label(), op);

        let res = match op {
            M3FSOperation::NEXT_IN => self.execute_on_session(input, |sess, is| sess.next_in(is)),
            M3FSOperation::NEXT_OUT => self.execute_on_session(input, |sess, is| sess.next_out(is)),
            M3FSOperation::COMMIT => self.execute_on_session(input, |sess, is| sess.commit(is)),
            M3FSOperation::CLOSE => {
                // Get session id, then notify caller that we closed, finally close self
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
        if let Some(sess) = self.get_session(session_id) {
            function(sess, is)
        }
        else {
            Err(Error::new(Code::InvArgs))
        }
    }

    fn get_session(&mut self, sess: SessId) -> Option<&mut FSSession> {
        if let Some(s) = self.sessions.get_mut(sess) {
            return Some(s);
        }

        None
    }

    fn close_session(&mut self, session_id: SessId) -> Result<(), Error> {
        log!(LOG_SESSION, "[{}] closing session", session_id);

        let (crt, file_session) = if let Some(sess) = self.sessions.get(session_id) {
            // Remove session from inner collection
            let crt = sess.creator();
            let mut fsess: Option<Rc<RefCell<FileSession>>> = None;
            if let FSSession::File(file_session) = sess {
                fsess = Some(file_session.clone());
            }

            (crt, fsess)
        }
        else {
            return Err(Error::new(Code::InvArgs)); // There was no session with the given Id registered
        };
        // remove session from inner container
        self.sessions.remove(crt, session_id);

        // if the removed session was a file session, clean up the open_files table and the parent meta session
        if let Some(fsess) = file_session {
            if let Some(ext) = fsess.borrow().append_ext.clone() {
                hdl()
                    .blocks()
                    .free(ext.start as usize, ext.length as usize)?;
            }
            // Delete append extent if there was any
            fsess.borrow_mut().append_ext = None;

            // Remove session from open_files and from its meta session
            hdl().files().remove_session(fsess.clone())?;

            // remove file session from parent meta session
            let parent_meta_session = self
                .sessions
                .get_mut(fsess.borrow().meta_session)
                .expect("Could not find file sessions parent meta session!");
            if let FSSession::Meta(ref mut pms) = parent_meta_session {
                pms.remove_file(fsess.clone());
            }
            else {
                log!(
                    LOG_DEF,
                    "FileSessions parents session was not a meta session!"
                );
            }

            // Revoke rights if needed
            if fsess.borrow().last != m3::kif::INVALID_SEL {
                m3::pes::VPE::cur().revoke(
                    m3::kif::CapRngDesc::new(m3::kif::CapType::OBJECT, fsess.borrow().last, 1),
                    false,
                )?;
            }
        }

        // Drop remaining messages for this id
        REQHDL.recv_gate().drop_msgs_with(session_id as Label);
        Ok(())
    }
}

impl Handler<FSSession> for M3FSRequestHandler {
    fn sessions(&mut self) -> &mut SessionContainer<FSSession> {
        &mut self.sessions
    }

    /// Creates a new session with `arg` as an argument for the service with selector `srv_sel`.
    /// Returns the session selector and the session identifier.
    fn open(
        &mut self,
        crt: usize,
        srv_sel: Selector,
        arg: &str,
    ) -> Result<(Selector, SessId), Error> {
        // Get max number of files
        let mut max_files: usize = 64;
        if arg.len() > 6 {
            if &arg[..6] == "file=" {
                max_files = arg[6..].parse().unwrap_or(64);
            }
        } // Otherwise there is an argument but it is not big enough

        // Get the id this session would belong to.
        let sessid = self.sessions.next_id()?;

        self.sessions.add_next(crt, srv_sel, true, |sess| {
            log!(crate::LOG_SESSION, "[{}] creating session", sess.ident());
            Ok(FSSession::Meta(MetaSession::new(
                sess, sessid, crt, max_files,
            )))
        })
    }

    /// Let's the client obtain a capability from the server
    fn obtain(&mut self, mut crt: usize, sid: SessId, data: &mut CapExchange) -> Result<(), Error> {
        // TODO hotfix for buggy crt mechanism, must be removed later

        if crt == 0 && self.in_memory && SHELL_HACK {
            println!(
                "M3FS-RS WARNING: changed obtain(crt) from 0 to 1 according to memory-backend specific hack"
            );
            crt += 1;
        }

        if !self.sessions.can_add(crt) {
            log!(
                LOG_DEF,
                "m3fs:obtain: WARNING: Can't add new session for creator: {}, this is a bug that needs to be fixed!",
                crt
            );
            return Err(Error::new(Code::NoSpace));
        }

        let next_sess_id = self.sessions.next_id()?;

        let sel: Selector = self.sel;

        let session = if let Some(s) = self.get_session(sid) {
            s
        }
        else {
            return Err(Error::new(Code::InvArgs));
        };
        match session {
            FSSession::Meta(meta) => {
                if data.in_args().size() == 0 {
                    log!(crate::LOG_DEF, "[{}] fs::get_sgate()", sid);
                    meta.get_sgate(data)
                }
                else {
                    log!(crate::LOG_DEF, "[{}] fs::open_file()", sid);
                    let session = meta.open_file(sel, crt, data, next_sess_id)?;

                    self.sessions
                        .add(crt, next_sess_id, FSSession::File(session))?;
                    Ok(())
                }
            },
            FSSession::File(file) => {
                if data.in_args().size() == 0 {
                    log!(crate::LOG_DEF, "[{}] fs::clone()", sid);
                    file.borrow_mut().clone(sel, data)
                }
                else {
                    log!(crate::LOG_DEF, "[{}] fs::get_mem()", sid);
                    file.borrow_mut().get_mem(data)
                }
            },
        }
    }

    /// Let's the client delegate a capability to the server
    fn delegate(&mut self, crt: usize, sid: SessId, data: &mut CapExchange) -> Result<(), Error> {
        log!(LOG_DEF, "[{}] fs::delegate()", sid);
        let session = if let Some(s) = self.get_session(sid) {
            s
        }
        else {
            log!(
                LOG_DEF,
                "fs::delegate: could not find session with id {}, crt: {}",
                sid,
                crt
            );
            return Err(Error::new(Code::NotSup));
        };

        if data.in_caps() != 1 || !session.is_file_session() {
            log!(
                LOG_DEF,
                "fs::delegate: was not file session or data_caps != 1"
            );
            return Err(Error::new(Code::NotSup));
        }

        if let FSSession::File(fs) = session {
            let new_sel: Selector = VPE::cur().alloc_sel();
            log!(
                LOG_DEF,
                "fs::delegate: set_ep(sel: {}, sid: {})",
                new_sel,
                sid
            );
            fs.borrow_mut().set_ep(new_sel);
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
}

#[derive(Clone, Debug)]
pub struct FsSettings {
    name: String,
    backend: String,
    fs_size: usize,
    extend: usize,
    max_load: usize,
    clear: bool,
    revoke_first: bool,
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
            revoke_first: false,
            selector: None,
            ep: EP_COUNT,
            fs_offset: FS_IMG_OFFSET,
        }
    }
}

fn usage() -> ! {
    println!(
        "Usage: {} [-n <name>] [-s <sel>] [-e <blocks>] [-c] [-r] [-b <blocks>]",
        env::args().next().unwrap()
    );
    println!("       [-o <offset>] (disk|mem <fssize>)");
    println!();
    println!("  -n: the name of the service (m3fs by default)");
    println!("  -s: don't create service, use selectors <sel>..<sel+1>");
    println!("  -e: the number of blocks to extend files when appending");
    println!("  -c: clear allocated blocks");
    println!("  -r: revoke first, reply afterwards");
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
            "-r" => {
                settings.revoke_first = true;
                i -= 1;
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

    // Create backend for the file system
    let mut hdl = match settings.backend.as_str() {
        "mem" => {
            let backend = MemBackend::new(settings.fs_offset, settings.fs_size);
            M3FSRequestHandler::new(backend, settings.clone())
                .expect("Failed to create m3fs handler based on memory backend")
        },
        "disk" | _ => {
            let backend = DiskBackend::new().expect("Failed to initialize disk backend!");
            M3FSRequestHandler::new(backend, settings.clone())
                .expect("Failed to create m3fs handler based on disk backend")
        },
    };

    // Create new server for file system and pass on selector to handler
    let serv = Server::new(&settings.name, &mut hdl).expect("Could not create service 'm3fs'");
    hdl.sel = serv.sel();

    // Create request handler
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
