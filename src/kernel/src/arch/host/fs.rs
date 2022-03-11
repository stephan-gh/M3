/*
 * Copyright (C) 2020-2021 Nils Asmussen, Barkhausen Institut
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

use base::cfg;
use base::libc;
use base::mem::MaybeUninit;

use crate::mem;

pub fn copy_from_fs(path: &str) -> usize {
    let in_path = base::format!("{}\0", path);
    unsafe {
        let fd = libc::open(in_path.as_bytes().as_ptr() as *const i8, libc::O_RDONLY);
        assert!(fd != -1);

        let mut info: libc::stat = MaybeUninit::uninit().assume_init();
        assert!(libc::fstat(fd, &mut info) != -1);
        assert!(info.st_size as usize <= cfg::FS_MAX_SIZE);

        let addr = mem::borrow_mut().mods()[0].addr().offset();
        let res = libc::read(fd, addr as *mut libc::c_void, info.st_size as usize);
        assert!(res == info.st_size as isize);

        libc::close(fd);

        let fs_size = res as usize;
        klog!(MEM, "Copied fs-image '{}' to 0..{:#x}", in_path, fs_size);
        fs_size
    }
}

pub fn copy_to_fs(path: &str, fs_size: usize) {
    let out_path = base::format!("{}.out\0", path);

    unsafe {
        let fd = libc::open(
            out_path.as_bytes().as_ptr() as *const i8,
            libc::O_WRONLY | libc::O_TRUNC | libc::O_CREAT,
            0o600,
        );
        assert!(fd != -1);

        let addr = mem::borrow_mut().mods()[0].addr().offset();
        libc::write(fd, addr as *const libc::c_void, fs_size);
        libc::close(fd);
    }

    klog!(MEM, "Copied fs-image back to '{}'", out_path);
}
