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

use base::cfg;
use base::col::ToString;
use base::env;
use base::envdata;
use base::errors::{Code, Error};
use base::format;
use base::kif;
use base::libc;
use base::tcu::{ActId, TileId};
use base::vec;

use crate::ktcu;
use crate::tiles::{tilemng, Activity};

pub fn start(act: &Activity) -> Result<i32, Error> {
    if let Some(pid) = act.pid() {
        write_env_file(pid, act.id(), act.tile_id(), 0);
        return Ok(pid);
    }

    let pid = unsafe { libc::fork() };
    match pid {
        -1 => Err(Error::new(Code::OutOfMem)),
        0 => {
            let pid = unsafe { libc::getpid() };
            write_env_file(pid, act.id(), act.tile_id(), act.first_sel());

            let kernel = env::args().next().unwrap();
            let builddir = kernel.rsplit_once('/').unwrap().0;

            let mut arg = builddir.to_string();
            arg.push('/');
            arg.push_str(act.name());
            arg.push('\0');

            klog!(ACTIVITIES, "Loading mod '{}':", act.name());

            let argv = vec![arg.as_ptr() as *const i8, core::ptr::null::<i8>()];
            unsafe {
                libc::execv(argv[0], argv.as_ptr());
                // special error code to let the WorkLoop delete the activity
                libc::exit(255);
            }
        },
        pid => Ok(pid),
    }
}

pub fn finish_start(act: &Activity) -> Result<(), Error> {
    // update all EPs (e.g., to allow parents to activate EPs for their childs)
    // set base for all receive EPs (do it for all, but it's just unused for the other types)
    tilemng::tilemux(act.tile_id()).update_eps()
}

fn write_env_file(pid: i32, id: ActId, tile: TileId, first_sel: kif::CapSel) {
    let path = format!("{}/{}\0", envdata::tmp_dir(), pid);
    let data = format!(
        "{}\n{}\n{}\n{}\n{}\n{}\n{}\n",
        "foo", // TODO SHM prefix
        tile,
        first_sel,
        kif::FIRST_FREE_SEL,
        id,
        ktcu::KSYS_EP,
        cfg::SYSC_RBUF_SIZE,
    );

    unsafe {
        let fd = libc::open(
            path.as_bytes().as_ptr() as *const i8,
            libc::O_WRONLY | libc::O_TRUNC | libc::O_CREAT,
            0o600,
        );
        assert!(fd != -1);
        libc::write(fd, data.as_ptr() as *const libc::c_void, data.len());
        libc::close(fd);
    }
}
