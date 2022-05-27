/*
 * Copyright (C) 2021 Nils Asmussen, Barkhausen Institut
 * Copyright (C) 2021, Tendsin Mende <tendsin.mende@mailbox.tu-dresden.de>
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

// hosts a simple fifo driver that is based on Unix sockets. More or less copies smoltcp's
// RawSocket. But implemented in no_std environment.

use core::default::Default;

use libc::{c_char, c_int, sockaddr_un};

use m3::col::Vec;
use m3::libc;
use m3::rc::Rc;
use m3::tcu::TCU;
use m3::{format, log, vec};

use smoltcp::phy::{Device, DeviceCapabilities};
use smoltcp::time::Instant;

fn get_socket(name: &str, suff: &str) -> sockaddr_un {
    let mut addr = sockaddr_un {
        sun_family: libc::AF_UNIX as u16,
        sun_path: [0; 108],
    };

    let formated = format!("\0m3_net_{}_{}\0", name, suff);
    for (i, c) in formated.as_bytes().iter().enumerate() {
        addr.sun_path[i] = *c as c_char;
    }

    addr
}

// Inner raw socket description, more or less copied from smoltcps's sys::RawSocket, but in no_std.
pub struct RawSocketDesc {
    in_fd: c_int,
    out_fd: c_int,
    out_socket: sockaddr_un,
}

impl RawSocketDesc {
    pub fn new(name: &str) -> Self {
        let in_fd = unsafe {
            let lower = libc::socket(libc::AF_UNIX, libc::SOCK_DGRAM, 0);
            if lower == -1 {
                panic!(
                    "Unix socket creation failed with {}",
                    (*libc::__errno_location()) as i32
                );
            }
            lower
        };
        let out_fd = unsafe {
            let lower = libc::socket(libc::AF_UNIX, libc::SOCK_DGRAM, 0);
            if lower == -1 {
                panic!(
                    "Unix socket creation failed with {}!",
                    (*libc::__errno_location()) as i32
                );
            }
            lower
        };

        log!(
            crate::LOG_NIC,
            "opened unix socket[{}] & socket[{}]",
            in_fd,
            out_fd,
        );

        // Bind socket
        let in_sock = get_socket(name, "in");
        unsafe {
            let res = libc::bind(
                in_fd,
                &in_sock as *const libc::sockaddr_un as *const libc::sockaddr,
                core::mem::size_of::<libc::sockaddr_un>() as u32,
            );
            if res == -1 {
                panic!(
                    "Failed to bind in_socket[{}] with error={}",
                    in_fd,
                    (*libc::__errno_location()) as i32
                );
            }
        }

        // if we wait, wake up if there is something to read from `in_fd`
        TCU::add_wait_fd(in_fd);

        let out_socket = get_socket(name, "out");
        RawSocketDesc {
            in_fd,
            out_fd,
            out_socket,
        }
    }

    pub fn recv(&self, buffer: &mut [u8]) -> Option<usize> {
        if buffer.is_empty() {
            return None;
        }

        unsafe {
            let len = libc::recvfrom(
                self.in_fd,
                buffer.as_mut_ptr() as *mut libc::c_void,
                buffer.len(),
                libc::MSG_DONTWAIT,
                core::ptr::null_mut() as *mut libc::sockaddr,
                core::ptr::null_mut() as *mut u32,
            );
            if len == -1 {
                let errc = (*libc::__errno_location()) as i32;
                if errc != libc::EWOULDBLOCK {
                    log!(crate::LOG_NIC, "receive failed with error={}", errc);
                }
                return None;
            }

            log!(crate::LOG_NIC, "received paket with {}b", len);
            Some(len as usize)
        }
    }

    pub fn send(&self, buffer: &[u8]) -> Option<usize> {
        unsafe {
            let len = libc::sendto(
                self.out_fd,
                buffer.as_ptr() as *const libc::c_void,
                buffer.len(),
                0,
                &self.out_socket as *const libc::sockaddr_un as *const libc::sockaddr,
                core::mem::size_of::<libc::sockaddr_un>() as u32,
            );
            if len == -1 {
                let errc = (*libc::__errno_location()) as i32;
                if errc != libc::EWOULDBLOCK {
                    log!(crate::LOG_NIC, "send failed with error={}", errc);
                }
                return None;
            }

            log!(crate::LOG_NIC, "sent paket with {}b", len);
            Some(len as usize)
        }
    }
}

impl Drop for RawSocketDesc {
    fn drop(&mut self) {
        log!(
            crate::LOG_NIC,
            "Closing unix socket[{}] & socket[{}]",
            self.in_fd,
            self.out_fd
        );

        unsafe {
            if libc::close(self.in_fd) != 0 {
                panic!("failed to close {}", self.in_fd);
            }
            if libc::close(self.out_fd) != 0 {
                panic!("failed to close {}", self.out_fd);
            }
        }
    }
}

/// Fifo wrapper around the RawSocketDesc.
pub struct DevFifo {
    lower: Rc<RawSocketDesc>,
    mtu: usize,
}

impl DevFifo {
    pub fn new(name: &str) -> Self {
        let lower = RawSocketDesc::new(name);
        DevFifo {
            lower: Rc::new(lower),
            mtu: 2048,
        }
    }

    pub fn needs_poll(&self) -> bool {
        false
    }
}

impl<'a> Device<'a> for DevFifo {
    type RxToken = RxToken;
    type TxToken = TxToken;

    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.max_transmission_unit = self.mtu;
        caps
    }

    fn receive(&'a mut self) -> Option<(Self::RxToken, Self::TxToken)> {
        let mut buffer = vec![0; self.mtu];
        self.lower.recv(&mut buffer[..]).map(|size| {
            buffer.resize(size, 0);
            let rx = RxToken { buffer };
            let tx = TxToken {
                lower: self.lower.clone(),
            };
            (rx, tx)
        })
    }

    fn transmit(&'a mut self) -> Option<Self::TxToken> {
        Some(TxToken {
            lower: self.lower.clone(),
        })
    }
}

pub struct RxToken {
    buffer: Vec<u8>,
}

impl smoltcp::phy::RxToken for RxToken {
    fn consume<R, F>(mut self, _timestamp: Instant, f: F) -> smoltcp::Result<R>
    where
        F: FnOnce(&mut [u8]) -> smoltcp::Result<R>,
    {
        f(&mut self.buffer[..])
    }
}

pub struct TxToken {
    lower: Rc<RawSocketDesc>,
}

impl smoltcp::phy::TxToken for TxToken {
    fn consume<R, F>(self, _timestamp: Instant, len: usize, f: F) -> smoltcp::Result<R>
    where
        F: FnOnce(&mut [u8]) -> smoltcp::Result<R>,
    {
        let mut buffer = vec![0; len];
        let res = f(&mut buffer)?;
        match self.lower.send(&buffer[..]) {
            Some(_) => Ok(res),
            None => Err(smoltcp::Error::Exhausted),
        }
    }
}
