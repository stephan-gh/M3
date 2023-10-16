/*
 * Copyright (C) 2023-2024, Stephan Gerhold <stephan@gerhold.net>
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

use base::serde::{Deserializer, Serializer};
use base::serialize::{Deserialize, Serialize};
use core::fmt;
use core::ops::{Deref, DerefMut};
use hex::FromHex;

#[repr(transparent)]
pub struct Hex<T>(pub T);

impl<T: AsRef<[u8]>> fmt::Display for Hex<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for b in self.0.as_ref() {
            write!(f, "{:02x}", b)?;
        }
        Ok(())
    }
}

impl<const N: usize> Hex<[u8; N]> {
    pub const fn new_zeroed() -> Self {
        Hex([0; N])
    }
}

impl<T> Deref for Hex<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for Hex<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T: AsRef<[u8]>> fmt::Debug for Hex<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl<T: AsRef<[u8]>> Serialize for Hex<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        hex::serialize(&self.0, serializer)
    }
}

impl<'de, T: FromHex> Deserialize<'de> for Hex<T>
where
    <T as FromHex>::Error: fmt::Display,
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        hex::deserialize(deserializer).map(|data| Hex(data))
    }
}
