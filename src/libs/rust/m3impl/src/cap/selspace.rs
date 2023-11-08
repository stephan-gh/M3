/*
 * Copyright (C) 2023 Nils Asmussen, Barkhausen Institut
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

use crate::cap::Selector;
use crate::cell::{Cell, LazyReadOnlyCell};

static SELSPACE: LazyReadOnlyCell<SelSpace> = LazyReadOnlyCell::default();

/// The manager for the capability selector space
pub struct SelSpace {
    pub(crate) next: Cell<Selector>,
}

impl SelSpace {
    /// Returns the `SelSpace` instance
    pub fn get() -> &'static SelSpace {
        SELSPACE.get()
    }

    /// Allocates a new capability selector and returns it.
    pub fn alloc_sel(&self) -> Selector {
        self.alloc_sels(1)
    }

    /// Allocates `count` new and contiguous capability selectors and returns the first one.
    pub fn alloc_sels(&self, count: u64) -> Selector {
        let next = self.next.get();
        self.next.set(next + count);
        next
    }
}

pub(crate) fn init() {
    let env = crate::env::get();
    SELSPACE.set(SelSpace {
        next: Cell::from(env.load_first_sel()),
    });
}
