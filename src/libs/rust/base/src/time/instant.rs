/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2021 Nils Asmussen, Barkhausen Institut
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

//! Contains time measurement functions

use core::fmt;
use core::ops::{Add, AddAssign, Sub, SubAssign};

use crate::arch::cpu;
use crate::tcu::TCU;
use crate::time::{CycleDuration, TimeDuration};

/// A generic measurement of time
pub trait Instant {
    type Duration: crate::time::Duration;

    /// Creates a new instant for the current point in time.
    fn now() -> Self;

    /// Returns the amount of time elapsed from another instant to this one.
    fn duration_since(&self, earlier: Self) -> Self::Duration;
}

/// A measurement of time, represented in nanoseconds. Useful in combination with [`TimeDuration`].
// inspired by std::time::Instant
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TimeInstant(u64);

impl TimeInstant {
    /// Returns an instant corresponding to "now".
    pub fn now() -> Self {
        Self::from_nanos(TCU::nanotime())
    }

    /// Creates a new time instant from the given number of nanoseconds.
    pub fn from_nanos(nanos: u64) -> Self {
        Self(nanos)
    }

    /// Returns the time instant in nanoseconds
    pub fn as_nanos(&self) -> u64 {
        self.0
    }

    /// Returns the amount of time elapsed from another instant to this one, or None if that instant
    /// is later than this one.
    pub fn checked_duration_since(&self, earlier: Self) -> Option<TimeDuration> {
        self.0.checked_sub(earlier.0).map(TimeDuration::from_nanos)
    }

    /// Returns the amount of time elapsed from another instant to this one.
    pub fn duration_since(&self, earlier: Self) -> TimeDuration {
        TimeDuration::from_nanos(
            self.0
                .checked_sub(earlier.0)
                .expect("supplied instant is later than self"),
        )
    }

    /// Returns the amount of time elapsed since this instant was created.
    pub fn elapsed(&self) -> TimeDuration {
        TimeDuration::from_nanos(Self::now().0 - self.0)
    }
}

impl Add<TimeDuration> for TimeInstant {
    type Output = TimeInstant;

    fn add(self, other: TimeDuration) -> TimeInstant {
        Self::from_nanos(
            self.0
                .checked_add(other.as_nanos() as u64)
                .expect("overflow when adding duration to instant"),
        )
    }
}

impl AddAssign<TimeDuration> for TimeInstant {
    fn add_assign(&mut self, other: TimeDuration) {
        self.0 += other.as_nanos() as u64;
    }
}

impl Sub<TimeDuration> for TimeInstant {
    type Output = TimeInstant;

    fn sub(self, other: TimeDuration) -> TimeInstant {
        Self::from_nanos(
            self.0
                .checked_sub(other.as_nanos() as u64)
                .expect("overflow when subtracting duration from instant"),
        )
    }
}

impl SubAssign<TimeDuration> for TimeInstant {
    fn sub_assign(&mut self, other: TimeDuration) {
        self.0 -= other.as_nanos() as u64;
    }
}

impl Sub<TimeInstant> for TimeInstant {
    type Output = TimeDuration;

    fn sub(self, other: TimeInstant) -> TimeDuration {
        self.duration_since(other)
    }
}

impl fmt::Debug for TimeInstant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ns", self.0)
    }
}

impl Instant for TimeInstant {
    type Duration = TimeDuration;

    fn now() -> Self {
        TimeInstant::now()
    }

    fn duration_since(&self, earlier: Self) -> TimeDuration {
        self.duration_since(earlier)
    }
}

/// A measurement of cycles. Useful in combination with [`CycleDuration`].
#[derive(Copy, Clone)]
pub struct CycleInstant(u64);

impl CycleInstant {
    /// Returns an instant corresponding to "now".
    pub fn now() -> Self {
        Self::from_cycles(cpu::elapsed_cycles())
    }

    /// Creates a new cycle instant from the given number of cycles.
    pub fn from_cycles(cycles: u64) -> Self {
        Self(cycles)
    }

    /// Returns the number of cycles.
    pub fn as_cycles(&self) -> u64 {
        self.0
    }

    /// Returns the amount of time elapsed from another instant to this one.
    pub fn duration_since(&self, earlier: Self) -> CycleDuration {
        CycleDuration::new(
            self.0
                .checked_sub(earlier.0)
                .expect("supplied instant is later than self"),
        )
    }

    /// Returns the amount of time elapsed since this instant was created.
    pub fn elapsed(&self) -> CycleDuration {
        CycleDuration::new(Self::now().0 - self.0)
    }
}

impl Instant for CycleInstant {
    type Duration = CycleDuration;

    fn now() -> Self {
        CycleInstant::now()
    }

    fn duration_since(&self, earlier: Self) -> CycleDuration {
        self.duration_since(earlier)
    }
}
