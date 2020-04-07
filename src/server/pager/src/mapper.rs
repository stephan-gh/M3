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

use m3::errors::Error;
use m3::goff;
use m3::kif::Perm;
use m3::pes::Mapper;
use m3::session::MapFlags;
use m3::session::Pager;
use m3::vfs;

use AddrSpace;

pub(crate) struct ChildMapper<'a> {
    aspace: &'a mut AddrSpace,
    has_virtmem: bool,
}

impl<'a> ChildMapper<'a> {
    pub fn new(aspace: &'a mut AddrSpace, has_virtmem: bool) -> Self {
        ChildMapper {
            aspace,
            has_virtmem,
        }
    }
}

impl<'a> Mapper for ChildMapper<'a> {
    fn map_file<'l>(
        &mut self,
        _pager: Option<&'l Pager>,
        file: &mut vfs::BufReader<vfs::FileRef>,
        foff: usize,
        virt: goff,
        len: usize,
        perm: Perm,
        flags: MapFlags,
    ) -> Result<bool, Error> {
        if self.has_virtmem {
            let sess = file.get_ref().borrow().session().unwrap();
            self.aspace
                .map_ds_with(virt, len as goff, foff as goff, perm, flags, sess)
                .map(|_| false)
        }
        else {
            Ok(true)
        }
    }

    fn map_anon<'l>(
        &mut self,
        _pager: Option<&'l Pager>,
        virt: goff,
        len: usize,
        perm: Perm,
        flags: MapFlags,
    ) -> Result<bool, Error> {
        if self.has_virtmem {
            self.aspace
                .map_anon_with(virt, len as goff, perm, flags)
                .map(|_| false)
        }
        else {
            // nothing to do
            Ok(true)
        }
    }
}
