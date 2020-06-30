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

use base::tcu;
use core::intrinsics;
use thread;

use com;
use ktcu;
use pes;
use syscalls;

pub fn thread_startup() {
    workloop();

    thread::ThreadManager::get().stop();
}

pub fn workloop() {
    let thmng = thread::ThreadManager::get();
    let vpemng = pes::vpemng::get();

    while vpemng.count() > 0 {
        tcu::TCU::sleep().unwrap();

        if let Some(msg) = ktcu::fetch_msg(ktcu::KSYS_EP) {
            syscalls::handle(msg);
        }

        if let Some(msg) = ktcu::fetch_msg(ktcu::KSRV_EP) {
            unsafe {
                let squeue: *mut com::SendQueue = intrinsics::transmute(msg.header.label as usize);
                (*squeue).received_reply(msg);
            }
        }

        #[cfg(target_os = "none")]
        if let Some(msg) = ktcu::fetch_msg(ktcu::KPEX_EP) {
            let pe = msg.header.label as usize;
            pes::pemng::get().pemux(pe).handle_call(msg);
        }

        thmng.try_yield();

        #[cfg(target_os = "linux")]
        ::arch::childs::check_childs();
        #[cfg(target_os = "linux")]
        ::arch::net::check();
    }
}
