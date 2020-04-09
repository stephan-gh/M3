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

use base::libc;
use base::tcu::TCU;
use core::ptr;
use core::sync::atomic;

use pes::vpemng;

static mut SIGCHLDS: atomic::AtomicUsize = atomic::AtomicUsize::new(0);

pub fn init() {
    unsafe {
        libc::signal(libc::SIGCHLD, sigchld_handler as usize);
    }

    TCU::bind_knotify();
}

pub fn kill_child(pid: i32) {
    unsafe {
        libc::kill(pid, libc::SIGTERM);
        libc::waitpid(pid, ptr::null_mut(), 0);
    }
}

pub fn check_childs() {
    if let Some((pid, status)) = TCU::receive_knotify() {
        kill_vpe(pid, status);
    }

    unsafe {
        while SIGCHLDS.load(atomic::Ordering::Relaxed) > 0 {
            SIGCHLDS.fetch_sub(1, atomic::Ordering::Relaxed);

            let mut status = 0;
            let pid = libc::wait(&mut status);
            if pid != -1 {
                kill_vpe(pid, status);
            }
        }
    }
}

fn kill_vpe(pid: libc::pid_t, status: i32) {
    let (vpe_id, vpe_name) = match vpemng::get().find_vpe(|v| v.pid() == pid) {
        Some(v) => {
            let id = v.id();
            (Some(id), format!("{}:{}", id, v.name()))
        },
        None => (None, format!("??")),
    };

    unsafe {
        if libc::WIFEXITED(status) {
            klog!(
                VPES,
                "Child {} exited with status {}",
                vpe_name,
                libc::WEXITSTATUS(status)
            );
        }
        else if libc::WIFSIGNALED(status) {
            klog!(
                VPES,
                "Child {} was killed by signal {}",
                vpe_name,
                libc::WTERMSIG(status)
            );
        }

        if libc::WIFSIGNALED(status) || libc::WEXITSTATUS(status) == 255 {
            if let Some(vid) = vpe_id {
                vpemng::get().remove(vid);
            }
        }
    }
}

extern "C" fn sigchld_handler(_sig: i32) {
    unsafe {
        SIGCHLDS.fetch_add(1, atomic::Ordering::Relaxed);
        libc::signal(libc::SIGCHLD, sigchld_handler as usize);
    }
}
