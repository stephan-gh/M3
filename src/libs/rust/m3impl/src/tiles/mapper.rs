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

//! The mapper types that are used to init the memory of an activity.

use crate::client::{MapFlags, Pager};
use crate::errors::{Code, Error};
use crate::kif;
use crate::mem::VirtAddr;
use crate::vfs::{BufReader, File, FileRef, Map};

/// The mapper trait is used to map the memory of an activity before running it.
pub trait Mapper {
    /// Maps the given file to `virt`..`virt`+`len` with given permissions.
    #[allow(clippy::too_many_arguments)]
    fn map_file(
        &mut self,
        pager: Option<&Pager>,
        file: &mut BufReader<FileRef<dyn File>>,
        foff: usize,
        virt: VirtAddr,
        len: usize,
        perm: kif::Perm,
        flags: MapFlags,
    ) -> Result<bool, Error>;

    /// Maps anonymous memory to `virt`..`virt`+`len` with given permissions.
    fn map_anon(
        &mut self,
        pager: Option<&Pager>,
        virt: VirtAddr,
        len: usize,
        perm: kif::Perm,
        flags: MapFlags,
    ) -> Result<bool, Error>;
}

/// The default implementation of the [`Mapper`] trait.
pub struct DefaultMapper {
    has_virtmem: bool,
}

impl DefaultMapper {
    /// Creates a new `DefaultMapper`.
    pub fn new(has_virtmem: bool) -> Self {
        DefaultMapper { has_virtmem }
    }
}

impl Mapper for DefaultMapper {
    fn map_file(
        &mut self,
        pager: Option<&Pager>,
        file: &mut BufReader<FileRef<dyn File>>,
        foff: usize,
        virt: VirtAddr,
        len: usize,
        perm: kif::Perm,
        flags: MapFlags,
    ) -> Result<bool, Error> {
        if let Some(pg) = pager {
            file.get_ref()
                .map(pg, virt, foff, len, perm, flags)
                .map(|_| false)
        }
        else if self.has_virtmem {
            // exec with VM, but without pager is not supported
            Err(Error::new(Code::NotSup))
        }
        else {
            Ok(true)
        }
    }

    fn map_anon(
        &mut self,
        pager: Option<&Pager>,
        virt: VirtAddr,
        len: usize,
        perm: kif::Perm,
        flags: MapFlags,
    ) -> Result<bool, Error> {
        if let Some(pg) = pager {
            pg.map_anon(virt, len, perm, flags).map(|_| false)
        }
        else if self.has_virtmem {
            // exec with VM, but without pager is not supported
            Err(Error::new(Code::NotSup))
        }
        else {
            Ok(true)
        }
    }
}
