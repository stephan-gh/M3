/*
 * Copyright (C) 2020-2022 Nils Asmussen, Barkhausen Institut
 * Copyright (C) 2019-2020, Tendsin Mende <tendsin@protonmail.com>
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

mod backend;
mod buf;
mod data;
mod ops;
mod sess;

use crate::backend::{Backend, DiskBackend, MemBackend};
use crate::buf::{FileBuffer, MetaBuffer};
use crate::data::{Allocator, SuperBlock};
use crate::sess::{FSSession, M3FSSession, OpenFiles};

use m3::server::ExcType;
use m3::{
    boxed::Box,
    cap::Selector,
    cell::{LazyReadOnlyCell, LazyStaticRefCell, LazyStaticUnsafeCell, Ref, RefMut, StaticRefCell},
    col::{String, ToString, Vec},
    com::opcodes,
    env,
    errors::{Code, Error},
    io::LogFlags,
    server::{RequestHandler, Server, DEF_MAX_CLIENTS},
    tiles::OwnActivity,
};

// Server constants
const MSG_SIZE: usize = 128;

static SB: LazyStaticRefCell<SuperBlock> = LazyStaticRefCell::default();
// TODO we unfortunately need to use an unsafe cell here at the moment, because the meta buffer is
// basicalled used in all modules, making it really hard to use something like a RefCell here.
static MB: LazyStaticUnsafeCell<MetaBuffer> = LazyStaticUnsafeCell::default();
static FB: LazyStaticRefCell<FileBuffer> = LazyStaticRefCell::default();
static FILES: StaticRefCell<OpenFiles> = StaticRefCell::new(OpenFiles::new());
static BA: LazyStaticRefCell<Allocator> = LazyStaticRefCell::default();
static IA: LazyStaticRefCell<Allocator> = LazyStaticRefCell::default();
static SETTINGS: LazyReadOnlyCell<FsSettings> = LazyReadOnlyCell::default();
static BACKEND: LazyStaticRefCell<Box<dyn Backend>> = LazyStaticRefCell::default();

fn superblock() -> Ref<'static, SuperBlock> {
    SB.borrow()
}
fn superblock_mut() -> RefMut<'static, SuperBlock> {
    SB.borrow_mut()
}
fn meta_buffer_mut() -> &'static mut MetaBuffer {
    // safety: see comment for MB
    unsafe { MB.get_mut() }
}
fn file_buffer_mut() -> RefMut<'static, FileBuffer> {
    FB.borrow_mut()
}
fn open_files_mut() -> RefMut<'static, OpenFiles> {
    FILES.borrow_mut()
}
fn blocks_mut() -> RefMut<'static, Allocator> {
    BA.borrow_mut()
}
fn inodes_mut() -> RefMut<'static, Allocator> {
    IA.borrow_mut()
}
fn settings() -> &'static FsSettings {
    SETTINGS.get()
}
fn backend_mut() -> RefMut<'static, Box<dyn Backend>> {
    BACKEND.borrow_mut()
}

fn flush_buffer() -> Result<(), Error> {
    crate::meta_buffer_mut().flush()?;
    crate::file_buffer_mut().flush()?;

    // update superblock and write it back to disk/memory
    let mut sb = crate::superblock_mut();
    let inodes = crate::inodes_mut();
    sb.update_inodebm(inodes.free_count(), inodes.first_free());
    let blocks = crate::blocks_mut();
    sb.update_blockbm(blocks.free_count(), blocks.first_free());
    sb.checksum = sb.get_checksum();
    crate::backend_mut().store_sb(&sb)
}

#[derive(Clone, Debug)]
pub struct FsSettings {
    name: String,
    backend: String,
    mem_mod: String,
    extend: usize,
    max_load: usize,
    max_clients: usize,
    clear: bool,
    selector: Option<Selector>,
}

impl core::default::Default for FsSettings {
    fn default() -> Self {
        FsSettings {
            name: String::from("m3fs"),
            backend: String::from("mem"),
            mem_mod: String::from("fs"),
            extend: 128,
            max_load: 128,
            max_clients: DEF_MAX_CLIENTS,
            clear: false,
            selector: None,
        }
    }
}

fn usage() -> ! {
    println!(
        "Usage: {} [-n <name>] [-s <sel>] [-e <blocks>] [-c] [-f <name>] [-b <blocks>]",
        env::args().next().unwrap()
    );
    println!("       [-m <clients>] (disk|mem)");
    println!();
    println!("  -n: the name of the service (m3fs by default)");
    println!("  -s: don't create service, use selectors <sel>..<sel+1>");
    println!("  -e: the number of blocks to extend files when appending");
    println!("  -c: clear allocated blocks");
    println!("  -b: the maximum number of blocks loaded from the disk");
    println!("  -m: the maximum number of clients (receive slots)");
    println!("  -f: the name of the FS boot module ('fs' by default)");
    OwnActivity::exit_with(Code::InvArgs);
}

fn parse_args() -> Result<FsSettings, String> {
    let mut settings = FsSettings::default();

    let args: Vec<&str> = env::args().collect();
    let mut i = 1;
    while i < args.len() {
        match args[i] {
            "-n" => settings.name = args[i + 1].to_string(),
            "-f" => settings.mem_mod = args[i + 1].to_string(),
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
            "-m" => {
                settings.max_clients = args[i + 1]
                    .parse::<usize>()
                    .map_err(|_| String::from("Failed to parse client count"))?;
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
        "mem" | "disk" => {},
        backend => return Err(format!("Unknown backend {}", backend)),
    }

    Ok(settings)
}

fn init_fs(mut backend: Box<dyn Backend>) {
    // init thread manager, otherwise the waiting within the file and meta buffer impl. panics.
    thread::init();

    let sb = backend.load_sb().expect("Unable to load super block");
    log!(LogFlags::FSInfo, "Loaded {:#?}", sb);

    BA.set(Allocator::new(
        String::from("Block"),
        sb.first_blockbm_block(),
        sb.first_free_block,
        sb.free_blocks,
        sb.total_blocks,
        sb.blockbm_blocks(),
        sb.block_size as usize,
    ));
    IA.set(Allocator::new(
        String::from("INodes"),
        sb.first_inodebm_block(),
        sb.first_free_inode,
        sb.free_inodes,
        sb.total_inodes,
        sb.inodebm_block(),
        sb.block_size as usize,
    ));

    // safety: we pass in a newly constructed MetaBuffer and have not initialized MB before
    unsafe {
        MB.set(MetaBuffer::new(sb.block_size as usize));
    }
    FB.set(FileBuffer::new(sb.block_size as usize));
    SB.set(sb);

    BACKEND.set(backend);
}

#[no_mangle]
pub fn main() -> Result<(), Error> {
    // parse arguments
    SETTINGS.set(parse_args().unwrap_or_else(|e| {
        println!("Invalid arguments: {}", e);
        usage();
    }));
    log!(LogFlags::FSInfo, "{:#?}", SETTINGS.get());

    // create and initialize backend for the file system
    let backend = if SETTINGS.get().backend == "mem" {
        Box::new(MemBackend::new(&SETTINGS.get().mem_mod)) as Box<dyn Backend>
    }
    else {
        Box::new(DiskBackend::new().expect("Failed to initialize disk backend!"))
            as Box<dyn Backend>
    };
    init_fs(backend);

    // create request handler and server
    // TODO just temporary: set a very high limit for client connections until we repair the way
    // the meta session is used.
    let mut hdl = RequestHandler::new_with(SETTINGS.get().max_clients, MSG_SIZE, 1024)
        .expect("Unable to create request handler");
    let mut srv =
        Server::new(&SETTINGS.get().name, &mut hdl).expect("Could not create service 'm3fs'");

    use opcodes::FileSystem;

    // register capability handler
    hdl.reg_cap_handler(FileSystem::Open, ExcType::Obt(2), FSSession::open);
    hdl.reg_cap_handler(FileSystem::GetMem, ExcType::Obt(1), FSSession::get_mem);
    hdl.reg_cap_handler(FileSystem::DelEP, ExcType::Del(1), FSSession::del_ep);
    hdl.reg_cap_handler(FileSystem::CloneFile, ExcType::Obt(2), FSSession::clone);
    hdl.reg_cap_handler(FileSystem::CloneMeta, ExcType::Obt(2), FSSession::clone);
    hdl.reg_cap_handler(FileSystem::SetDest, ExcType::Del(1), FSSession::set_dest);
    hdl.reg_cap_handler(
        FileSystem::EnableNotify,
        ExcType::Del(1),
        FSSession::enable_notify,
    );

    // register message handler
    hdl.reg_msg_handler(FileSystem::NextIn, FSSession::next_in);
    hdl.reg_msg_handler(FileSystem::NextOut, FSSession::next_out);
    hdl.reg_msg_handler(FileSystem::Commit, FSSession::commit);
    hdl.reg_msg_handler(FileSystem::Truncate, FSSession::truncate);
    hdl.reg_msg_handler(FileSystem::Close, FSSession::close);
    hdl.reg_msg_handler(FileSystem::FStat, FSSession::stat);
    hdl.reg_msg_handler(FileSystem::GetPath, FSSession::get_path);
    hdl.reg_msg_handler(FileSystem::Seek, FSSession::seek);
    hdl.reg_msg_handler(FileSystem::Sync, FSSession::sync);
    hdl.reg_msg_handler(FileSystem::Stat, FSSession::fstat);
    hdl.reg_msg_handler(FileSystem::Mkdir, FSSession::mkdir);
    hdl.reg_msg_handler(FileSystem::Rmdir, FSSession::rmdir);
    hdl.reg_msg_handler(FileSystem::Link, FSSession::link);
    hdl.reg_msg_handler(FileSystem::Unlink, FSSession::unlink);
    hdl.reg_msg_handler(FileSystem::Rename, FSSession::rename);
    hdl.reg_msg_handler(FileSystem::OpenPriv, FSSession::open_priv);

    hdl.run(&mut srv).expect("Server loop failed");

    Ok(())
}
