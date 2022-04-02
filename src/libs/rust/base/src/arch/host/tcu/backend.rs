/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2022 Nils Asmussen, Barkhausen Institut
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

use alloc::{format, vec};
use core::ptr;
use libc;

use crate::arch::envdata;
use crate::arch::tcu::{thread, EpId, Header, TileId, TILE_COUNT, TOTAL_EPS};
use crate::col::Vec;
use crate::mem;

pub struct FdSet {
    set: libc::fd_set,
    max: i32,
}

impl FdSet {
    pub fn new() -> FdSet {
        unsafe {
            let mut raw_fd_set = mem::MaybeUninit::<libc::fd_set>::uninit();
            libc::FD_ZERO(raw_fd_set.as_mut_ptr());
            FdSet {
                set: raw_fd_set.assume_init(),
                max: 0,
            }
        }
    }

    pub fn set(&mut self, fd: i32) {
        unsafe { libc::FD_SET(fd, &mut self.set) }
        self.max = core::cmp::max(self.max, fd);
    }

    pub fn is_set(&self, fd: i32) -> bool {
        unsafe { libc::FD_ISSET(fd, &self.set) }
    }
}

struct UnixSocket {
    fd: i32,
    addr: libc::sockaddr_un,
}

impl UnixSocket {
    fn new(fd: i32, addr: libc::sockaddr_un) -> Self {
        Self { fd, addr }
    }

    fn bind(&self) {
        unsafe {
            assert!(
                libc::bind(
                    self.fd,
                    &self.addr as *const _ as *const libc::sockaddr,
                    mem::size_of::<libc::sockaddr_un>() as u32
                ) != -1
            );
        }
    }

    fn send<T>(&self, data: T) {
        unsafe {
            let res = libc::sendto(
                self.fd,
                &data as *const _ as *const libc::c_void,
                mem::size_of_val(&data),
                0,
                &self.addr as *const _ as *const libc::sockaddr,
                mem::size_of::<libc::sockaddr_un>() as u32,
            );
            assert!(res != -1);
        }
    }

    fn receive<T>(&self, data: &mut T, block: bool) -> bool {
        let res = unsafe {
            libc::recvfrom(
                self.fd,
                data as *mut _ as *mut libc::c_void,
                mem::size_of_val(data),
                if block { 0 } else { libc::MSG_DONTWAIT },
                ptr::null_mut(),
                ptr::null_mut(),
            )
        };
        res > 0
    }
}

pub(crate) struct SocketBackend {
    sock: i32,
    cmd_sock: UnixSocket,
    ack_sock: UnixSocket,
    knotify_sock: UnixSocket,
    localsock: Vec<i32>,
    eps: Vec<libc::sockaddr_un>,
    add_fds: Vec<i32>,
}

#[repr(C, packed)]
#[derive(Default)]
struct KNotifyData {
    pid: libc::pid_t,
    status: i32,
}

impl SocketBackend {
    fn get_sock_addr(addr: &str) -> libc::sockaddr_un {
        let mut sockaddr = libc::sockaddr_un {
            sun_family: libc::AF_UNIX as libc::sa_family_t,
            sun_path: [0; 108],
        };
        sockaddr.sun_path[0..addr.len()]
            .clone_from_slice(unsafe { &*(addr.as_bytes() as *const [u8] as *const [i8]) });
        sockaddr
    }

    fn create_sock(name: &str) -> UnixSocket {
        let sock = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_DGRAM, 0) };
        assert!(sock != -1);
        unsafe {
            assert!(libc::fcntl(sock, libc::F_SETFD, libc::FD_CLOEXEC) == 0);
        }
        let sock_name = format!("\0{}/{}\0", envdata::tmp_dir(), name);
        UnixSocket::new(sock, Self::get_sock_addr(&sock_name))
    }

    fn ep_idx(tile: TileId, ep: EpId) -> usize {
        tile as usize * TOTAL_EPS as usize + ep as usize
    }

    pub fn new() -> SocketBackend {
        let sock = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_DGRAM, 0) };
        assert!(sock != -1);

        let tile = envdata::get().tile_id as TileId;
        let cmd_sock = Self::create_sock(&format!("tile{}-cmd", tile));
        cmd_sock.bind();
        let ack_sock = Self::create_sock(&format!("tile{}-ack", tile));
        ack_sock.bind();
        let knotify_sock = Self::create_sock("knotify");

        let mut eps = vec![];
        for tile in 0..TILE_COUNT {
            for ep in 0..TOTAL_EPS {
                let addr = format!("\0{}/ep_{}.{}\0", envdata::tmp_dir(), tile, ep);
                eps.push(Self::get_sock_addr(&addr));
            }
        }

        let mut localsock = vec![];
        for ep in 0..TOTAL_EPS {
            unsafe {
                let epsock = libc::socket(libc::AF_UNIX, libc::SOCK_DGRAM, 0);
                assert!(epsock != -1);

                assert!(libc::fcntl(epsock, libc::F_SETFD, libc::FD_CLOEXEC) == 0);

                assert!(
                    libc::bind(
                        epsock,
                        &eps[Self::ep_idx(tile, ep)] as *const libc::sockaddr_un
                            as *const libc::sockaddr,
                        mem::size_of::<libc::sockaddr_un>() as u32
                    ) == 0
                );

                localsock.push(epsock);
            }
        }

        SocketBackend {
            sock,
            cmd_sock,
            ack_sock,
            knotify_sock,
            localsock,
            eps,
            add_fds: Vec::new(),
        }
    }

    pub fn add_wait_fd(&mut self, fd: i32) {
        self.add_fds.push(fd);
    }

    pub fn send(&self, tile: TileId, ep: EpId, buf: &thread::Buffer) -> bool {
        let sock = &self.eps[Self::ep_idx(tile, ep)];
        let res = unsafe {
            libc::sendto(
                self.sock,
                buf as *const thread::Buffer as *const libc::c_void,
                buf.header.length + mem::size_of::<Header>(),
                0,
                sock as *const libc::sockaddr_un as *const libc::sockaddr,
                mem::size_of::<libc::sockaddr_un>() as u32,
            )
        };
        res != -1
    }

    pub fn receive(&self, ep: EpId, buf: &mut thread::Buffer) -> Option<usize> {
        let res = unsafe {
            libc::recvfrom(
                self.localsock[ep as usize],
                buf as *mut thread::Buffer as *mut libc::c_void,
                mem::size_of::<thread::Buffer>(),
                libc::MSG_DONTWAIT,
                ptr::null_mut(),
                ptr::null_mut(),
            )
        };
        if res <= 0 {
            None
        }
        else {
            Some(res as usize)
        }
    }

    pub fn wait_for_work(&self, timeout: Option<u64>) -> bool {
        // build fd sets; one for reading, one for error
        let mut fds = [FdSet::new(), FdSet::new()];
        for f in &mut fds {
            f.set(self.cmd_sock.fd);
            f.set(self.knotify_sock.fd);
            for fd in &self.localsock {
                f.set(*fd);
            }
            for fd in &self.add_fds {
                f.set(*fd);
            }
        }

        // build timeout
        let mut timeout_spec = timeout.map(|to| libc::timespec {
            tv_nsec: (to % 1_000_000_000) as i64,
            tv_sec: (to / 1_000_000_000) as i64,
        });

        let res = unsafe {
            libc::pselect(
                fds[0].max + 1,
                &mut fds[0].set as *mut _,
                ptr::null_mut(),
                &mut fds[1].set as *mut _,
                match timeout_spec {
                    Some(ref mut p) => p as *mut _,
                    None => ptr::null_mut(),
                },
                ptr::null_mut(),
            )
        };

        // check whether any additional fd became ready
        let mut add_ready = false;
        if res != -1 {
            // for the kernel: wake up if a notification was received
            if fds[0].is_set(self.knotify_sock.fd) {
                add_ready = true;
            }
            for fd in &self.add_fds {
                if fds[0].is_set(*fd) {
                    add_ready = true;
                }
            }
        }
        add_ready
    }

    pub fn send_command(&self) {
        self.cmd_sock.send(0u8);
    }

    pub fn recv_command(&self) -> bool {
        self.cmd_sock.receive(&mut 0u8, false)
    }

    pub fn send_ack(&self) {
        self.ack_sock.send(0u8)
    }

    pub fn recv_ack(&self) -> bool {
        // block until the ACK for the command arrived
        self.ack_sock.receive(&mut 0u8, true)
    }

    pub fn bind_knotify(&self) {
        self.knotify_sock.bind();
    }

    pub fn notify_kernel(&self, pid: libc::pid_t, status: i32) {
        let data = KNotifyData { pid, status };
        self.knotify_sock.send(data);
    }

    pub fn receive_knotify(&self) -> Option<(libc::pid_t, i32)> {
        let mut data = KNotifyData::default();
        if self.knotify_sock.receive(&mut data, false) {
            Some((data.pid, data.status))
        }
        else {
            None
        }
    }

    pub fn shutdown(&self) {
        for ep in 0..TOTAL_EPS {
            unsafe { libc::shutdown(self.localsock[ep as usize], libc::SHUT_RD) };
        }
    }
}

impl Drop for SocketBackend {
    fn drop(&mut self) {
        for ep in 0..TOTAL_EPS {
            unsafe { libc::close(self.localsock[ep as usize]) };
        }
    }
}
