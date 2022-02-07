/*
 * Copyright (C) 2020 Nils Asmussen, Barkhausen Institut
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

use core::fmt;
use core::ops::Deref;
use core::ptr;
use core::ptr::NonNull;

use crate::boxed::Box;
use crate::cell::Cell;

struct SRcBox<T: ?Sized> {
    refs: Cell<usize>,
    value: T,
}

/// Simple reference counter that does not support weak references
pub struct SRc<T: ?Sized> {
    ptr: NonNull<SRcBox<T>>,
}

impl<T> SRc<T> {
    /// Creates a new `SRc` with given object
    pub fn new(value: T) -> Self {
        Self {
            ptr: Box::leak(Box::new(SRcBox {
                refs: Cell::new(1),
                value,
            }))
            .into(),
        }
    }
}

impl<T: ?Sized> Deref for SRc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &(*self.ptr.as_ptr()).value }
    }
}

impl<T> Clone for SRc<T> {
    fn clone(&self) -> Self {
        let inner = self.ptr.as_ptr();
        unsafe {
            (*inner).refs.set((*inner).refs.get() + 1);
            Self {
                ptr: NonNull::new_unchecked(inner),
            }
        }
    }
}

impl<T: ?Sized> Drop for SRc<T> {
    #[inline(always)]
    fn drop(&mut self) {
        let inner = self.ptr.as_ptr();
        unsafe {
            if (*inner).refs.get() == 1 {
                ptr::drop_in_place(self.ptr.as_mut());
            }
            else {
                (*inner).refs.set((*inner).refs.get() - 1);
            }
        }
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for SRc<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}
