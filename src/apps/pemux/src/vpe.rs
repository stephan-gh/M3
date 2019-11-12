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

use base::cell::StaticCell;

pub struct VPE {
    id: u64,
}

static CUR: StaticCell<Option<VPE>> = StaticCell::new(None);

pub fn add(id: u64) {
    assert!((*CUR).is_none());

    log!(PEX_VPES, "Created VPE {}", id);
    CUR.set(Some(VPE::new(id)));
}

pub fn remove() {
    if (*CUR).is_some() {
        log!(PEX_VPES, "Destroyed VPE {}", (*CUR).as_ref().unwrap().id);
        CUR.set(None);
    }
}

impl VPE {
    pub fn new(id: u64) -> Self {
        VPE { id }
    }
}
