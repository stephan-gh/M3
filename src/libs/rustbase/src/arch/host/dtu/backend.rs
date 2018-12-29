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

use arch::dtu::*;
use col::Vec;
use core::intrinsics;
use libc;
use util;

pub(crate) struct SocketBackend {
    sock: i32,
    pending: i32,
    localsock: Vec<i32>,
    fds: Vec<libc::pollfd>,
    eps: Vec<libc::sockaddr_un>,
}

int_enum! {
    /// The system calls
    pub struct Event : u64 {
        const REQ     = 0;
        const RESP    = 1;
        const MSG     = 2;
    }
}

impl SocketBackend {
    pub fn new() -> SocketBackend {
        let sock = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_DGRAM, 0) };
        assert!(sock != -1);

        let mut eps = vec![];
        for pe in 0..PE_COUNT {
            for ep in 0..EP_COUNT + 3 {
                let addr = format!("\0m3_ep_{}.{}\0", pe, ep);
                let mut sockaddr = libc::sockaddr_un {
                    sun_family: libc::AF_UNIX as libc::sa_family_t,
                    sun_path: [0; 108],
                };
                sockaddr.sun_path[0..addr.len()].clone_from_slice(
                    unsafe { intrinsics::transmute(addr.as_bytes()) }
                );
                eps.push(sockaddr);
            }
        }

        let pe = arch::envdata::get().pe_id;
        let mut localsock = vec![];
        for ep in 0..EP_COUNT + 3 {
            unsafe {
                let epsock = libc::socket(libc::AF_UNIX, libc::SOCK_DGRAM, 0);
                assert!(epsock != -1);

                assert!(libc::fcntl(epsock, libc::F_SETFD, libc::FD_CLOEXEC) == 0);
                assert!(libc::fcntl(epsock, libc::F_SETFL, libc::O_NONBLOCK) == 0);

                assert!(libc::bind(
                    epsock,
                    intrinsics::transmute(&eps[pe as usize * (EP_COUNT + 3) + ep]),
                    util::size_of::<libc::sockaddr_un>() as u32
                ) == 0);

                localsock.push(epsock);
            }
        }

        let mut fds = vec![];
        for i in 0..EP_COUNT + 1 {
            let fd = libc::pollfd {
                fd: localsock[i],
                events: libc::POLLIN | libc::POLLERR,
                revents: 0,
            };
            fds.push(fd);
        }

        SocketBackend {
            sock: sock,
            pending: 0,
            localsock: localsock,
            fds: fds,
            eps: eps,
        }
    }

    fn poll(&mut self) {
        self.pending = unsafe {
            libc::ppoll(
                self.fds[0..EP_COUNT + 1].as_mut_ptr(),
                self.fds.len() as u64,
                ptr::null_mut(),
                ptr::null_mut()
            )
        };
        // assert!(self.pending >= 0 || libc::errno == libc::EINTR);
    }

    pub fn has_command(&mut self) -> Option<()> {
        if self.pending <= 0 {
            self.poll();
        }

        let fdidx = EP_COUNT + Event::REQ.val as usize;
        if self.fds[fdidx].revents != 0 {
            unsafe {
                let mut dummy: u8 = 0;
                let res = libc::recvfrom(
                    self.fds[fdidx].fd,
                    &mut dummy as *mut u8 as *mut libc::c_void,
                    1,
                    0,
                    ptr::null_mut(),
                    ptr::null_mut(),
                );
                assert!(res != -1);
            }

            self.fds[fdidx].revents = 0;
            self.pending -= 1;
            Some(())
        }
        else {
            None
        }
    }

    pub fn has_msg(&mut self) -> Option<EpId> {
        if self.pending <= 0 {
            self.poll();
        }

        for ep in 0..EP_COUNT {
            if self.fds[ep].revents != 0 {
                self.fds[ep].revents = 0;
                self.pending -= 1;
                return Some(ep);
            }
        }

        None
    }

    pub fn notify(&self, ev: Event) {
        let pe = arch::envdata::get().pe_id as usize;
        let dummy: u8 = 0;

        unsafe {
            let sock = &self.eps[pe * (EP_COUNT + 3) + EP_COUNT + ev.val as usize];
            let res = libc::sendto(
                self.sock,
                &dummy as *const u8 as *const libc::c_void,
                1,
                0,
                sock as *const libc::sockaddr_un as *const libc::sockaddr,
                util::size_of::<libc::sockaddr_un>() as u32
            );
            assert!(res != -1);
        }
    }

    pub fn wait(&self, ev: Event) -> bool {
        let mut fds = libc::pollfd {
            fd: self.localsock[EP_COUNT + ev.val as usize],
            events: libc::POLLIN | libc::POLLERR | libc::POLLHUP,
            revents: 0,
        };
        let res = unsafe {
            libc::ppoll(
                &mut fds as *mut libc::pollfd,
                1,
                ptr::null_mut(),
                ptr::null_mut()
            )
        };
        if res == -1 {
            return false
        }

        let res = unsafe {
            let mut dummy: u8 = 0;
            libc::recvfrom(
                fds.fd,
                &mut dummy as *mut u8 as *mut libc::c_void,
                1,
                0,
                ptr::null_mut(),
                ptr::null_mut(),
            )
        };
        res != -1
    }

    pub fn send(&self, pe: PEId, ep: EpId, buf: &thread::Buffer) -> bool {
        unsafe {
            let sock = &self.eps[pe * (EP_COUNT + 3) + ep];
            let res = libc::sendto(
                self.sock,
                buf as *const thread::Buffer as *const libc::c_void,
                buf.header.length + util::size_of::<Header>(),
                0,
                sock as *const libc::sockaddr_un as *const libc::sockaddr,
                util::size_of::<libc::sockaddr_un>() as u32
            );
            res != -1
        }
    }

    pub fn receive(&self, ep: EpId, buf: &mut thread::Buffer) -> Option<usize> {
        unsafe {
            let res = libc::recvfrom(
                self.localsock[ep],
                buf as *mut thread::Buffer as *mut libc::c_void,
                util::size_of::<thread::Buffer>(),
                0,
                ptr::null_mut(),
                ptr::null_mut(),
            );
            if res <= 0 {
                None
            }
            else {
                Some(res as usize)
            }
        }
    }

    pub fn shutdown(&self) {
        for ep in 0..EP_COUNT {
            unsafe { libc::shutdown(self.localsock[ep], libc::SHUT_RD) };
        }
    }
}

impl Drop for SocketBackend {
    fn drop(&mut self) {
        for ep in 0..EP_COUNT {
            unsafe { libc::close(self.localsock[ep]) };
        }
    }
}
