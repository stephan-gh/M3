/*
 * Copyright (C) 2021-2022 Nils Asmussen, Barkhausen Institut
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

//! Contains the quota type that is passed around for information purposes

use crate::kif::tilemux;
use crate::serialize::{Deserialize, Serialize};

use core::fmt;

/// A quota identifier
pub type Id = tilemux::QuotaId;

/// Represents a generic quota, consisting of an id, a total amount, and a remaining amount
#[derive(Copy, Clone, Default, Serialize, Deserialize)]
pub struct Quota<T> {
    id: Id,
    total: T,
    remaining: T,
}

impl<T: Copy> Quota<T> {
    /// Creates a new Quota with given id, total budget and remaining budget
    pub fn new(id: Id, total: T, left: T) -> Self {
        Self {
            id,
            total,
            remaining: left,
        }
    }

    /// Returns the quota id
    pub fn id(&self) -> Id {
        self.id
    }

    /// Returns the total budget
    pub fn total(&self) -> T {
        self.total
    }

    /// Returns the remaining budget
    pub fn remaining(&self) -> T {
        self.remaining
    }
}

impl<T: fmt::Debug> fmt::Debug for Quota<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Q[{}: {:?} of {:?}]",
            self.id, self.remaining, self.total
        )
    }
}
