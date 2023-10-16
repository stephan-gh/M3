/*
 * Copyright (C) 2024, Stephan Gerhold <stephan@gerhold.net>
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

#[repr(transparent)]
pub struct Secret<T> {
    pub secret: T,
}

impl<T> Secret<T> {
    pub const fn new(secret: T) -> Self {
        Self { secret }
    }
}

impl<const N: usize> Secret<[u8; N]> {
    pub const fn new_zeroed() -> Self {
        Self::new([0; N])
    }
}

impl<T: AsRef<[u8]>> fmt::Debug for Secret<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let bits = self.secret.as_ref().len() * u8::BITS as usize;
        write!(f, "Secret(REDACTED {} bits..)", bits)
        //write!(
        //    f,
        //    "Secret(INSECURE {} bits: {})",
        //    bits,
        //    crate::hex::Hex(self.secret.as_ref())
        //)
    }
}

impl<T> Drop for Secret<T> {
    fn drop(&mut self) {
        unsafe { base::util::clear_volatile(&mut self.secret as *mut T) }
    }
}
