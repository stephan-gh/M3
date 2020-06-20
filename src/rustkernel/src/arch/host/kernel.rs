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

use base::env;
use base::envdata;
use base::goff;
use base::io;
use base::kif;
use base::libc;
use base::mem::heap;
use base::tcu;
use thread;

use super::{fs, net};
use arch::loader;
use args;
use ktcu;
use mem;
use pes;
use platform;
use workloop::{thread_startup, workloop};

#[no_mangle]
pub extern "C" fn rust_init(argc: i32, argv: *const *const i8) {
    envdata::set(envdata::EnvData::new(
        0,
        kif::PEDesc::new(kif::PEType::COMP_IMEM, kif::PEISA::X86, 1024 * 1024),
        argc,
        argv,
        0,
        0,
    ));
    heap::init();
    io::init(0, "rkernel");
    tcu::init();
}

#[no_mangle]
pub extern "C" fn rust_deinit(_status: i32, _arg: *const libc::c_void) {
    tcu::deinit();
}

#[no_mangle]
pub fn main() -> i32 {
    args::parse();

    unsafe {
        libc::mkdir("/tmp/m3\0".as_ptr() as *const i8, 0o755);
    }

    mem::init();
    ktcu::init();
    platform::init(&args::get().free);
    let kernel = env::args().next().unwrap();
    let builddir = kernel.rsplitn(2, '/').nth(1).unwrap();
    loader::init(&builddir);
    ::arch::childs::init();

    thread::init();
    for _ in 0..8 {
        thread::ThreadManager::get().add_thread(thread_startup as *const () as usize, 0);
    }

    pes::pemng::init();
    pes::vpemng::init();

    let fs_size = if let Some(ref path) = args::get().fs_image {
        fs::copy_from_fs(path)
    }
    else {
        0
    };
    if let Some(bname) = args::get().net_bridge.as_ref() {
        net::create_bridge(bname);
    }

    let sysc_rbuf = vec![0u8; 512 * 32];
    ktcu::recv_msgs(ktcu::KSYS_EP, sysc_rbuf.as_ptr() as goff, 14, 9)
        .expect("Unable to config syscall REP");

    let serv_rbuf = vec![0u8; 1024];
    ktcu::recv_msgs(ktcu::KSRV_EP, serv_rbuf.as_ptr() as goff, 10, 8)
        .expect("Unable to config service REP");

    let vpemng = pes::vpemng::get();
    vpemng.start_root().expect("starting root failed");

    klog!(DEF, "Kernel is ready!");

    workloop();

    pes::vpemng::deinit();
    if let Some(ref path) = args::get().fs_image {
        fs::copy_to_fs(path, fs_size);
    }

    klog!(DEF, "Shutting down");
    0
}
