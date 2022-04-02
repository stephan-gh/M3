/*
 * Copyright (C) 2020-2022 Nils Asmussen, Barkhausen Institut
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

use base::col::ToString;
use base::format;
use base::libc;
use base::rc::Rc;
use base::tcu::TCU;
use core::ptr;
use core::sync::atomic;

use crate::tiles::{Activity, ActivityMng, State};

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

pub fn check_childs_async() {
    if let Some((pid, status)) = TCU::receive_knotify() {
        kill_activity_async(pid, status);
    }

    unsafe {
        while SIGCHLDS.load(atomic::Ordering::Relaxed) > 0 {
            SIGCHLDS.fetch_sub(1, atomic::Ordering::Relaxed);

            let mut status = 0;
            let pid = libc::wait(&mut status);
            if pid != -1 {
                kill_activity_async(pid, status);
            }
        }
    }
}

fn kill_activity_async(pid: libc::pid_t, status: i32) {
    let (act, act_name) =
        match ActivityMng::find_activity(|v: &Rc<Activity>| v.pid().unwrap_or(0) == pid) {
            Some(v) => {
                let id = v.id();
                let name = format!("{}:{}", id, v.name());
                (Some(v), name)
            },
            None => (None, "??".to_string()),
        };

    if libc::WIFEXITED(status) {
        klog!(
            ACTIVITIES,
            "Child {} exited with status {}",
            act_name,
            libc::WEXITSTATUS(status)
        );
    }
    else if libc::WIFSIGNALED(status) {
        klog!(
            ACTIVITIES,
            "Child {} was killed by signal {}",
            act_name,
            libc::WTERMSIG(status)
        );
    }

    if libc::WIFSIGNALED(status) || libc::WEXITSTATUS(status) == 255 {
        if let Some(v) = act {
            // only remove the activity if it has an app; otherwise the kernel sent the signal
            if v.state() == State::RUNNING {
                ActivityMng::remove_activity_async(v.id());
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
