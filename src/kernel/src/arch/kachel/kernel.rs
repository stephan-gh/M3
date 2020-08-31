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

use base::goff;
use base::io;
use base::machine;
use base::math;
use base::mem::heap;
use thread;

use crate::arch::{exceptions, loader, paging};
use crate::args;
use crate::ktcu;
use crate::mem;
use crate::pes;
use crate::platform;
use crate::workloop::{thread_startup, workloop};

#[no_mangle]
pub extern "C" fn abort() -> ! {
    exit(1);
}

#[no_mangle]
pub extern "C" fn exit(_code: i32) -> ! {
    machine::shutdown();
}

#[no_mangle]
pub extern "C" fn env_run() {
    exceptions::init();
    heap::init();
    io::init(0, "kernel");
    crate::slab::init();
    paging::init();
    mem::init();

    args::parse();

    platform::init(&[]);
    loader::init();

    thread::init();
    for _ in 0..8 {
        thread::ThreadManager::get().add_thread(thread_startup as *const () as usize, 0);
    }

    pes::init();

    // TODO add second syscall REP
    let sysc_rbuf = vec![0u8; 512 * 32];
    ktcu::recv_msgs(ktcu::KSYS_EP, sysc_rbuf.as_ptr() as goff, 14, 9)
        .expect("Unable to config syscall REP");

    let serv_rbuf = vec![0u8; 1024];
    ktcu::recv_msgs(ktcu::KSRV_EP, serv_rbuf.as_ptr() as goff, 10, 8)
        .expect("Unable to config service REP");

    let pex_rbuf_ord = math::next_log2(32) + 7;
    let pex_rbuf = vec![0u8; 1 << pex_rbuf_ord];
    ktcu::recv_msgs(ktcu::KPEX_EP, pex_rbuf.as_ptr() as goff, pex_rbuf_ord, 7)
        .expect("Unable to config pemux REP");

    pes::VPEMng::get().start_root().expect("starting root failed");

    klog!(DEF, "Kernel is ready!");

    workloop();

    pes::deinit();

    klog!(DEF, "Shutting down");
    exit(0);
}
