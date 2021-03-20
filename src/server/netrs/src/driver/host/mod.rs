/*
 * Copyright (C) 2021, Tendsin Mende <tendsin.mende@mailbox.tu-dresden.de>
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

// hosts a simple fifo driver that is based on Unix sockets. More or less copies smoltcp's
// RawSocket. But implemented in no_std environment.

use core::default::Default;

use libc::{c_char, c_int, sockaddr_un};

use log::info;

use m3::cell::RefCell;
use m3::col::Vec;
use m3::libc;
use m3::rc::Rc;
use m3::{log, vec, format};

use smoltcp::phy::{Device, DeviceCapabilities};
use smoltcp::time::Instant;

use crate::sess::socket_session::TCP_BUFFER_SIZE;

fn get_socket(name: &str, suff: &str) -> sockaddr_un {
    let mut addr = sockaddr_un {
        sun_family: libc::AF_UNIX as u16,
        sun_path: [0; 108],
    };

    // Note: I'm note sure why we need to start here with the \0. However, if we don't,
    // we cant send over this address does not exist. That's correct since
    // now only one, shared socket with name "" gets created in each service.
    let formated = format!("\0m3_net_{}_{}\0", name, suff);
    info!("Get socket {}", formated);

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
    name: &'static str,
}

impl RawSocketDesc {
    pub fn new(name: &'static str) -> Result<Self, ()> {
        let in_fd = unsafe {
            let lower = libc::socket(libc::AF_UNIX, libc::SOCK_DGRAM, 0);

            if lower == -1 {
                info!(
                    "Unix socket creation failed with {}!",
                    (*libc::__errno_location()) as i32
                );
                return Err(());
            }
            lower
        };
        let out_fd = unsafe {
            let lower = libc::socket(libc::AF_UNIX, libc::SOCK_DGRAM, 0);

            if lower == -1 {
                info!(
                    "Unix socket creation failed with {}!",
                    (*libc::__errno_location()) as i32
                );
                return Err(());
            }
            lower
        };

        // Bind socket
        let in_sock = get_socket(name, "in");
        unsafe {
            let res = libc::bind(
                in_fd,
                &in_sock as *const libc::sockaddr_un as *const libc::sockaddr,
                core::mem::size_of::<libc::sockaddr_un>() as u32,
            );
            if res == -1 {
                info!(
                    "Failed to bind in_socket[{}] with error={}",
                    in_fd,
                    (*libc::__errno_location()) as i32
                );
                return Err(());
            }
        }

        let out_socket = get_socket(name, "out");

        Ok(RawSocketDesc {
            in_fd,
            out_fd,
            out_socket,
            name,
        })
    }

    pub fn recv(&mut self, buffer: &mut [u8]) -> Result<usize, ()> {
        // log!(crate::LOG_NIC, "recv for buffer of size={}", buffer.len());
        if buffer.len() <= 0 {
            return Err(());
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
                // TODO handle would block error.

                let errc = (*libc::__errno_location()) as i32;
                if errc == 11 {
                    // Would block ignore that error
                    // log!(crate::LOG_NIC, "Would block");
                }
                else {
                    log!(
                        crate::LOG_NIC,
                        "Failed to recv on socket[{}] for buffer of len={} with error={}",
                        self.in_fd,
                        buffer.len(),
                        errc
                    );
                }
                return Err(());
            }
            log!(crate::LOG_NIC, "Got package of len {}", len);
            Ok(len as usize)
        }
    }

    pub fn send(&mut self, buffer: &[u8]) -> Result<usize, ()> {
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
                if errc == 11 {
                    log!(crate::LOG_NIC, "SEND: Would block");
                }

                // TODO handle would block error
                log!(
                    crate::LOG_NIC,
                    "Failed to send on socket[{}] buffer of len={} with error={}",
                    self.out_fd,
                    buffer.len(),
                    errc
                );
                return Err(());
            }
            Ok(len as usize)
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
                log!(crate::LOG_NIC, "Failed to close in_fd={}", self.in_fd);
            }
            if libc::close(self.out_fd) != 0 {
                log!(crate::LOG_NIC, "Failed to close out_fd={}", self.out_fd);
            }

            // Delete in file
            let formated_string = format!("m3_net_{}_in\0", self.name);

            let mut c_char_name = [0 as c_char; 108];

            for (i, c) in formated_string.as_bytes().iter().enumerate() {
                c_char_name[i] = *c as c_char;
            }
            if libc::remove(&c_char_name as *const _) == -1 {
                log!(
                    crate::LOG_NIC,
                    "Failed to delete socket {} with error={}",
                    self.name,
                    (*libc::__errno_location()) as i32
                );
            }
        }
    }
}

/// Fifo wrapper around the RawSocketDesc.
pub struct DevFifo {
    lower: Rc<RefCell<RawSocketDesc>>,
    mtu: usize,
}

impl<'a> DevFifo {
    pub fn new(name: &'static str) -> Result<Self, ()> {
        let lower = RawSocketDesc::new(name)?;
        Ok(DevFifo {
            lower: Rc::new(RefCell::new(lower)),
            mtu: TCP_BUFFER_SIZE,
        })
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
        let mut lower = self.lower.borrow_mut();
        let mut buffer = vec![0; self.mtu];
        match lower.recv(&mut buffer[..]) {
            Ok(size) => {
                buffer.resize(size, 0);
                let rx = RxToken { buffer };
                let tx = TxToken {
                    lower: self.lower.clone(),
                };
                Some((rx, tx))
            },
            Err(_err) => None,
        }
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
    lower: Rc<RefCell<RawSocketDesc>>,
}

impl smoltcp::phy::TxToken for TxToken {
    fn consume<R, F>(self, _timestamp: Instant, len: usize, f: F) -> smoltcp::Result<R>
    where
        F: FnOnce(&mut [u8]) -> smoltcp::Result<R>,
    {
        let mut lower = self.lower.borrow_mut();
        let mut buffer = vec![0; len];
        let res = f(&mut buffer)?;
        if let Err(_) = lower.send(&buffer[..]) {
            panic!("Could not send package");
        }
        else {
            Ok(res)
        }
    }
}
