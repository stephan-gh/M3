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

use base::cell::LazyStaticCell;
use base::libc;
use base::mem;
use base::tcu::{EpId, PEId, INVALID_EP, TCU};

use crate::ktcu;

static DEST: LazyStaticCell<(PEId, EpId)> = LazyStaticCell::default();

pub fn init() {
    // don't configure stdin if it's no terminal
    if unsafe { libc::isatty(libc::STDIN_FILENO) } == 0 {
        return;
    }

    // make stdin non-blocking
    unsafe {
        let flags = libc::fcntl(libc::STDIN_FILENO, libc::F_GETFL, 0);
        assert!(flags != -1);
        libc::fcntl(libc::STDIN_FILENO, libc::F_SETFL, flags | libc::O_NONBLOCK);
    }

    // enter raw mode
    let mut termios = unsafe {
        let mut termios = core::mem::zeroed();
        assert!(libc::tcgetattr(libc::STDIN_FILENO, &mut termios) != -1);
        termios
    };
    termios.c_lflag &= !(libc::ICANON | libc::ISIG | libc::ECHO);
    termios.c_cc[libc::VMIN] = 1;
    termios.c_cc[libc::VTIME] = 0;
    unsafe {
        assert!(libc::tcsetattr(libc::STDIN_FILENO, libc::TCSAFLUSH, &termios) != -1);
    }

    klog!(DEF, "Entered raw mode; Quit via Ctrl+]");

    // wake up if there is anything to read
    TCU::add_wait_fd(libc::STDIN_FILENO);
}

pub fn start(dest: Option<(PEId, EpId)>) {
    if let Some((pe, ep)) = dest {
        DEST.set((pe, ep));
    }
}

pub fn check() {
    if unsafe { libc::isatty(libc::STDIN_FILENO) } == 0 {
        return;
    }

    // read multiple bytes to get sequences like ^[D
    let mut buf = [0u8; 8];
    let res = unsafe {
        libc::read(
            libc::STDIN_FILENO,
            buf.as_mut_ptr() as *mut libc::c_void,
            mem::size_of_val(&buf),
        )
    };
    if res > 0 {
        // stop on ctrl+]
        if res == 1 && buf[0] == 0x1d {
            unsafe {
                libc::exit(0)
            };
        }

        if DEST.is_some() {
            // send to defined receive EP; ignore failures (e.g., no space)
            let (dest_pe, dest_ep) = DEST.get();
            let mut msg_buf = mem::MsgBuf::new();
            msg_buf.set_from_slice(&buf[0..res as usize]);
            ktcu::send_to(dest_pe, dest_ep, 0, &msg_buf, 0, INVALID_EP).ok();
        }
    }
}
