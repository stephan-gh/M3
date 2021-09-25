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

use base::cell::{LazyStaticCell, StaticRefCell};
use base::col::{String, ToString, Vec};
use base::format;
use base::libc;
use base::mem;
use base::tcu::TCU;
use core::ptr;

static BUF: StaticRefCell<[u8; 2048]> = StaticRefCell::new([0u8; 2048]);
static BR1: LazyStaticCell<Bridge> = LazyStaticCell::default();
static BR2: LazyStaticCell<Bridge> = LazyStaticCell::default();

struct Bridge {
    src_fd: i32,
    dst_fd: i32,
    dst_sock: libc::sockaddr_un,
}

fn get_sock_addr(addr: &str) -> libc::sockaddr_un {
    let mut sockaddr = libc::sockaddr_un {
        sun_family: libc::AF_UNIX as libc::sa_family_t,
        sun_path: [0; 108],
    };
    sockaddr.sun_path[0..addr.len()]
        .clone_from_slice(unsafe { &*(addr.as_bytes() as *const [u8] as *const [i8]) });
    sockaddr
}

impl Bridge {
    fn new(from: String, to: String) -> Self {
        let src_fd = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_DGRAM, 0) };
        assert!(src_fd != -1);
        let dst_fd = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_DGRAM, 0) };
        assert!(dst_fd != -1);

        let dst_sock = get_sock_addr(&format!("\0m3_net_{}", to));

        let src_sock = get_sock_addr(&format!("\0m3_net_{}", from));
        unsafe {
            assert!(
                libc::bind(
                    src_fd,
                    &src_sock as *const _ as *const libc::sockaddr,
                    mem::size_of::<libc::sockaddr_un>() as u32
                ) == 0,
                "Failed to bind socket: errno={}",
                (*libc::__errno_location()) as i32
            );
        }

        Self {
            src_fd,
            dst_fd,
            dst_sock,
        }
    }

    fn check(&self) {
        let mut buf = BUF.borrow_mut();

        let res = unsafe {
            libc::recvfrom(
                self.src_fd,
                buf.as_mut_ptr() as *mut libc::c_void,
                buf.len(),
                libc::MSG_DONTWAIT,
                ptr::null_mut(),
                ptr::null_mut(),
            )
        };
        if res <= 0 {
            return;
        }

        unsafe {
            assert!(
                libc::sendto(
                    self.dst_fd,
                    buf.as_ptr() as *const libc::c_void,
                    res as usize,
                    0,
                    &self.dst_sock as *const _ as *const libc::sockaddr,
                    mem::size_of::<libc::sockaddr_un>() as u32,
                ) != -1
            )
        };
    }
}

pub fn create_bridge(names: &str) {
    let parts: Vec<&str> = names.split('-').collect();
    assert!(parts.len() == 2);

    BR1.set(Bridge::new(
        parts[0].to_string() + "_out",
        parts[1].to_string() + "_in",
    ));
    BR2.set(Bridge::new(
        parts[1].to_string() + "_out",
        parts[0].to_string() + "_in",
    ));

    // wake up if there is anything to read
    TCU::add_wait_fd(BR1.src_fd);
    TCU::add_wait_fd(BR2.src_fd);
}

pub fn check() {
    if BR1.is_some() {
        BR1.check();
        BR2.check();
    }
}
