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

use cell::StaticCell;
use core::ops::Deref;

/// A `LazyStaticCell` is the same as the `StaticCell`, but contains an `Option<T>`. At
/// construction, the value is `None` and it needs to be set before other functions can be used.
/// That is, all access functions assume that the value has been set before.
pub struct LazyStaticCell<T: Sized> {
    inner: StaticCell<Option<T>>,
}

impl<T> LazyStaticCell<T> {
    pub const fn default() -> Self {
        Self {
            inner: StaticCell::new(None),
        }
    }

    /// Returns true if the value has been set
    pub fn is_some(&self) -> bool {
        self.inner.is_some()
    }

    /// Returns a reference to the inner value
    pub fn get(&self) -> &T {
        self.inner.get().as_ref().unwrap()
    }

    /// Returns a mutable reference to the inner value
    #[allow(clippy::mut_from_ref)]
    pub fn get_mut(&self) -> &mut T {
        self.inner.get_mut().as_mut().unwrap()
    }

    /// Sets the inner value to `val` and returns the old value
    pub fn set(&self, val: T) -> Option<T> {
        self.inner.set(Some(val)).map(|v| v)
    }

    /// Removes the inner value and returns the old value
    pub fn unset(&self) -> Option<T> {
        self.inner.set(None)
    }
}

impl<T: Sized> Deref for LazyStaticCell<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.get()
    }
}
