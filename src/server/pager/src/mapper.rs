/*
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

use m3::client::MapFlags;
use m3::client::Pager;
use m3::errors::Error;
use m3::kif::Perm;
use m3::mem::{GlobOff, VirtAddr};
use m3::tiles::Mapper;
use m3::vfs::{BufReader, File, FileRef};

use crate::AddrSpace;

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
    fn map_file(
        &mut self,
        _pager: Option<&Pager>,
        file: &mut BufReader<FileRef<dyn File>>,
        foff: usize,
        virt: VirtAddr,
        len: usize,
        perm: Perm,
        flags: MapFlags,
    ) -> Result<bool, Error> {
        if self.has_virtmem {
            let sess = file.get_ref().session().unwrap();
            self.aspace
                .map_ds_with(virt, len as GlobOff, foff as GlobOff, perm, flags, sess)
                .map(|_| false)
        }
        else {
            Ok(true)
        }
    }

    fn map_anon(
        &mut self,
        _pager: Option<&Pager>,
        virt: VirtAddr,
        len: usize,
        perm: Perm,
        flags: MapFlags,
    ) -> Result<bool, Error> {
        if self.has_virtmem {
            self.aspace
                .map_anon_with(virt, len as GlobOff, perm, flags)
                .map(|_| false)
        }
        else {
            // nothing to do
            Ok(true)
        }
    }
}
