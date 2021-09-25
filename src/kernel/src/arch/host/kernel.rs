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

use base::cell::StaticCell;
use base::cfg;
use base::env;
use base::envdata;
use base::goff;
use base::io;
use base::kif;
use base::libc;
use base::math;
use base::mem::heap;
use base::tcu;
use base::vec;
use thread;

use super::{fs, net};
use crate::arch::loader;
use crate::args;
use crate::ktcu;
use crate::pes;
use crate::platform;
use crate::workloop::workloop;

static FS_SIZE: StaticCell<usize> = StaticCell::new(0);

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
    crate::slab::init();
    io::init(0, "kernel");
    tcu::init();
}

#[no_mangle]
pub extern "C" fn rust_deinit(_status: i32, _arg: *const libc::c_void) {
    tcu::deinit();
}

#[no_mangle]
pub fn main() -> i32 {
    args::parse();

    ktcu::init();
    platform::init(&args::get().free);
    let kernel = env::args().next().unwrap();
    let builddir = kernel.rsplitn(2, '/').nth(1).unwrap();
    loader::init(&builddir);
    crate::arch::childs::init();
    crate::arch::input::init();
    crate::com::init_queues();

    thread::init();
    pes::init();

    FS_SIZE.set(if let Some(ref path) = args::get().fs_image {
        fs::copy_from_fs(path)
    }
    else {
        0
    });
    if let Some(bname) = args::get().net_bridge.as_ref() {
        net::create_bridge(bname);
    }

    let sysc_slot_size = 9;
    let sysc_rbuf_size = math::next_log2(cfg::MAX_VPES) + sysc_slot_size;
    let sysc_rbuf = vec![0u8; 1 << sysc_rbuf_size];
    ktcu::recv_msgs(
        ktcu::KSYS_EP,
        sysc_rbuf.as_ptr() as goff,
        sysc_rbuf_size,
        sysc_slot_size,
    )
    .expect("Unable to config syscall REP");

    let serv_slot_size = 8;
    let serv_rbuf_size = math::next_log2(crate::com::MAX_PENDING_MSGS) + serv_slot_size;
    let serv_rbuf = vec![0u8; 1 << serv_rbuf_size];
    ktcu::recv_msgs(
        ktcu::KSRV_EP,
        serv_rbuf.as_ptr() as goff,
        serv_rbuf_size,
        serv_slot_size,
    )
    .expect("Unable to config service REP");

    klog!(DEF, "Kernel is ready!");

    workloop();
}

pub fn shutdown() -> ! {
    pes::deinit();
    if let Some(ref path) = args::get().fs_image {
        fs::copy_to_fs(path, FS_SIZE.get());
    }
    klog!(DEF, "Shutting down");
    unsafe {
        libc::exit(0)
    };
}
