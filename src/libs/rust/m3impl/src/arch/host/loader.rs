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

use core::ptr;

use crate::col::{String, Vec};
use crate::errors::{Code, Error};
use crate::format;
use crate::io::Read;
use crate::libc;
use crate::mem::{size_of, MaybeUninit};
use crate::vec;
use crate::vfs::{File, FileRef};

pub struct Channel {
    fds: [i32; 2],
}

impl Channel {
    pub fn new() -> Result<Channel, Error> {
        let mut fds = [0i32; 2];
        match unsafe { libc::pipe(fds.as_mut_ptr()) } {
            -1 => Err(Error::new(Code::InvArgs)),
            _ => Ok(Channel { fds }),
        }
    }

    pub fn fds(&self) -> &[i32] {
        &self.fds
    }

    pub fn wait(&mut self) {
        unsafe {
            libc::close(self.fds[1]);
            self.fds[1] = -1;

            // wait until parent notifies us
            libc::read(self.fds[0], [0u8; 1].as_mut_ptr() as *mut libc::c_void, 1);
            libc::close(self.fds[0]);
            self.fds[0] = -1;
        }
    }

    pub fn signal(&mut self) {
        unsafe {
            libc::close(self.fds[0]);
            self.fds[0] = -1;

            // notify child; it can start now
            libc::write(self.fds[1], [0u8; 1].as_ptr() as *const libc::c_void, 1);
            libc::close(self.fds[1]);
            self.fds[1] = -1;
        }
    }
}

impl Drop for Channel {
    fn drop(&mut self) {
        unsafe {
            if self.fds[0] != -1 {
                libc::close(self.fds[0]);
            }
            if self.fds[1] != -1 {
                libc::close(self.fds[1]);
            }
        }
    }
}

pub fn copy_file(file: &mut FileRef<dyn File>) -> Result<String, Error> {
    let mut buf = vec![0u8; 8192];

    let mut path = format!("{}/exec-XXXXXX\0", base::envdata::tmp_dir());

    unsafe {
        let tmp = libc::mkstemp(path.as_bytes_mut().as_mut_ptr() as *mut i8);
        if tmp < 0 {
            return Err(Error::new(Code::InvArgs));
        }

        // copy executable from m3fs to a temp file
        loop {
            let res = file.read(&mut buf)?;
            if res == 0 {
                break;
            }

            libc::write(tmp, buf.as_ptr() as *const libc::c_void, res);
        }

        // close writable fd to make it non-busy
        libc::close(tmp);
    }

    Ok(path)
}

pub fn read_env_words(suffix: &str) -> Option<Vec<u64>> {
    read_env_file(suffix, |fd, size| {
        let mut res: Vec<u64> = vec![0; size as usize / 8];
        unsafe { libc::read(fd, res.as_mut_ptr() as *mut libc::c_void, size) };
        res
    })
}

pub fn read_env_file<F, R>(suffix: &str, func: F) -> Option<R>
where
    F: FnOnce(i32, usize) -> R,
{
    unsafe {
        let path = format!(
            "{}/{}-{}\0",
            base::envdata::tmp_dir(),
            libc::getpid(),
            suffix
        );
        let path_ptr = path.as_bytes().as_ptr() as *const i8;
        let fd = libc::open(path_ptr, libc::O_RDONLY);
        if fd == -1 {
            return None;
        }

        #[allow(clippy::uninit_assumed_init)]
        let mut info: libc::stat = MaybeUninit::uninit().assume_init();
        assert!(libc::fstat(fd, &mut info) != -1);
        let size = info.st_size as usize;
        assert!(size.trailing_zeros() >= 3);

        let res = func(fd, size);

        libc::unlink(path_ptr);

        libc::close(fd);
        Some(res)
    }
}

pub fn write_env_values(pid: i32, suffix: &str, data: &[u64]) {
    write_env_file(
        pid,
        suffix,
        data.as_ptr() as *const libc::c_void,
        data.len() * size_of::<u64>(),
    );
}

pub fn write_env_file(pid: i32, suffix: &str, data: *const libc::c_void, len: usize) {
    let path = format!("{}/{}-{}\0", base::envdata::tmp_dir(), pid, suffix);
    unsafe {
        let fd = libc::open(
            path.as_bytes().as_ptr() as *const i8,
            libc::O_WRONLY | libc::O_TRUNC | libc::O_CREAT,
            0o600,
        );
        assert!(fd != -1);
        libc::write(fd, data, len);
        libc::close(fd);
    }
}

pub fn exec<S: AsRef<str>>(args: &[S], path: &str) -> ! {
    let mut buf = vec![0u8; 4096];

    unsafe {
        // copy args and null-terminate them
        let mut argv: Vec<*const i8> = Vec::new();
        buf.set_len(0);
        for arg in args {
            let ptr = buf.as_slice()[buf.len()..].as_ptr();
            buf.extend_from_slice(arg.as_ref().as_bytes());
            buf.push(b'\0');
            argv.push(ptr as *const i8);
        }
        argv.push(ptr::null());

        // open it readonly again as fexecve requires
        let path_ptr = path.as_bytes().as_ptr() as *const i8;
        let tmpdup = libc::open(path_ptr, libc::O_RDONLY);
        // we don't need it anymore afterwards
        libc::unlink(path_ptr);
        // it needs to be executable
        libc::fchmod(tmpdup, 0o700);

        // execute that file
        extern "C" {
            static environ: *const *const i8;
        }
        libc::fexecve(tmpdup, argv.as_ptr(), environ);
        libc::_exit(1);
    }
}
