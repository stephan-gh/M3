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

use core::fmt;
use core::num::ParseIntError;
use core::ops;

use crate::goff;
use crate::serialize::{Deserialize, Serialize};

pub type VirtAddrRaw = u64;

#[derive(Default, Copy, Clone, Eq, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct VirtAddr(VirtAddrRaw);

impl VirtAddr {
    pub const fn null() -> Self {
        Self(0)
    }

    pub const fn new(addr: VirtAddrRaw) -> Self {
        Self(addr)
    }

    pub const fn as_raw(&self) -> VirtAddrRaw {
        self.0
    }

    pub const fn as_ptr<T>(&self) -> *const T {
        self.0 as *const T
    }

    pub const fn as_mut_ptr<T>(&self) -> *mut T {
        self.0 as *mut T
    }

    pub const fn as_goff(&self) -> goff {
        self.0 as goff
    }

    pub const fn as_local(&self) -> usize {
        self.0 as usize
    }

    pub const fn is_null(&self) -> bool {
        self.0 == 0
    }
}

impl From<usize> for VirtAddr {
    fn from(addr: usize) -> Self {
        Self(addr as VirtAddrRaw)
    }
}

impl<T> From<*const T> for VirtAddr {
    fn from(addr: *const T) -> Self {
        Self(addr as VirtAddrRaw)
    }
}

impl<T> From<*mut T> for VirtAddr {
    fn from(addr: *mut T) -> Self {
        Self(addr as VirtAddrRaw)
    }
}

impl fmt::Display for VirtAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "V[{:#x}]", self.0)
    }
}

impl fmt::Debug for VirtAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "V[{:#x}]", self.0)
    }
}

impl num_traits::Saturating for VirtAddr {
    fn saturating_add(self, v: Self) -> Self {
        Self(self.0.saturating_add(v.0))
    }

    fn saturating_sub(self, v: Self) -> Self {
        Self(self.0.saturating_sub(v.0))
    }
}

impl ops::Div for VirtAddr {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        Self(self.0 / rhs.0)
    }
}

impl num_traits::CheckedDiv for VirtAddr {
    fn checked_div(&self, v: &Self) -> Option<Self> {
        self.0.checked_div(v.0).map(|v| Self(v))
    }
}

impl ops::Rem for VirtAddr {
    type Output = Self;

    fn rem(self, rhs: Self) -> Self::Output {
        Self(self.0 % rhs.0)
    }
}

impl ops::Mul for VirtAddr {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self(self.0 * rhs.0)
    }
}

impl num_traits::CheckedMul for VirtAddr {
    fn checked_mul(&self, v: &Self) -> Option<Self> {
        self.0.checked_mul(v.0).map(|v| Self(v))
    }
}

impl ops::Add<usize> for VirtAddr {
    type Output = Self;

    fn add(self, rhs: usize) -> Self::Output {
        Self(self.0 + (rhs as VirtAddrRaw))
    }
}

impl ops::Add<goff> for VirtAddr {
    type Output = Self;

    fn add(self, rhs: goff) -> Self::Output {
        Self(self.0 + (rhs as VirtAddrRaw))
    }
}

impl ops::Add<VirtAddr> for VirtAddr {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl ops::AddAssign<usize> for VirtAddr {
    fn add_assign(&mut self, rhs: usize) {
        self.0 += rhs as VirtAddrRaw;
    }
}

impl num_traits::CheckedAdd for VirtAddr {
    fn checked_add(&self, v: &Self) -> Option<Self> {
        self.0.checked_add(v.0).map(|v| Self(v))
    }
}

impl ops::Sub<usize> for VirtAddr {
    type Output = Self;

    fn sub(self, rhs: usize) -> Self::Output {
        Self(self.0 - (rhs as VirtAddrRaw))
    }
}

impl ops::Sub<VirtAddr> for VirtAddr {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl num_traits::CheckedSub for VirtAddr {
    fn checked_sub(&self, v: &Self) -> Option<Self> {
        self.0.checked_sub(v.0).map(|v| Self(v))
    }
}

impl ops::Shr<usize> for VirtAddr {
    type Output = Self;

    fn shr(self, rhs: usize) -> Self::Output {
        Self(self.0 >> rhs)
    }
}

impl ops::Shl<usize> for VirtAddr {
    type Output = Self;

    fn shl(self, rhs: usize) -> Self::Output {
        Self(self.0 << rhs)
    }
}

impl ops::BitXor<Self> for VirtAddr {
    type Output = Self;

    fn bitxor(self, rhs: Self) -> Self::Output {
        Self(self.0 ^ rhs.0)
    }
}

impl ops::BitOr<Self> for VirtAddr {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl ops::BitAnd<Self> for VirtAddr {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl ops::Not for VirtAddr {
    type Output = Self;

    fn not(self) -> Self::Output {
        Self(!self.0)
    }
}

impl num_traits::Bounded for VirtAddr {
    fn min_value() -> Self {
        Self(VirtAddrRaw::min_value())
    }

    fn max_value() -> Self {
        Self(VirtAddrRaw::max_value())
    }
}

impl num_traits::ToPrimitive for VirtAddr {
    fn to_i64(&self) -> Option<i64> {
        self.0.to_i64()
    }

    fn to_u64(&self) -> Option<u64> {
        self.0.to_u64()
    }
}

impl num_traits::NumCast for VirtAddr {
    fn from<T: num_traits::ToPrimitive>(n: T) -> Option<Self> {
        <VirtAddrRaw as num_traits::NumCast>::from(n).map(|v| Self(v))
    }
}

impl num_traits::Zero for VirtAddr {
    fn zero() -> Self {
        Self(0)
    }

    fn is_zero(&self) -> bool {
        self.is_null()
    }
}

impl num_traits::One for VirtAddr {
    fn one() -> Self {
        Self(1)
    }
}

impl num_traits::Num for VirtAddr {
    type FromStrRadixErr = ParseIntError;

    fn from_str_radix(str: &str, radix: u32) -> Result<Self, Self::FromStrRadixErr> {
        VirtAddrRaw::from_str_radix(str, radix).map(|v| Self(v))
    }
}

impl num_traits::PrimInt for VirtAddr {
    fn count_ones(self) -> u32 {
        self.0.count_ones()
    }

    fn count_zeros(self) -> u32 {
        self.0.count_zeros()
    }

    fn leading_ones(self) -> u32 {
        self.0.leading_ones()
    }

    fn leading_zeros(self) -> u32 {
        self.0.leading_zeros()
    }

    fn trailing_ones(self) -> u32 {
        self.0.trailing_ones()
    }

    fn trailing_zeros(self) -> u32 {
        self.0.trailing_zeros()
    }

    fn rotate_left(self, n: u32) -> Self {
        Self(self.0.rotate_left(n))
    }

    fn rotate_right(self, n: u32) -> Self {
        Self(self.0.rotate_right(n))
    }

    fn signed_shl(self, n: u32) -> Self {
        Self(self.0.signed_shl(n))
    }

    fn signed_shr(self, n: u32) -> Self {
        Self(self.0.signed_shr(n))
    }

    fn unsigned_shl(self, n: u32) -> Self {
        Self(self.0.unsigned_shl(n))
    }

    fn unsigned_shr(self, n: u32) -> Self {
        Self(self.0.unsigned_shr(n))
    }

    fn swap_bytes(self) -> Self {
        Self(self.0.swap_bytes())
    }

    fn reverse_bits(self) -> Self {
        Self(self.0.reverse_bits())
    }

    fn from_be(x: Self) -> Self {
        Self(<VirtAddrRaw as num_traits::PrimInt>::from_be(x.0))
    }

    fn from_le(x: Self) -> Self {
        Self(<VirtAddrRaw as num_traits::PrimInt>::from_le(x.0))
    }

    fn to_be(self) -> Self {
        Self(self.0.to_be())
    }

    fn to_le(self) -> Self {
        Self(self.0.to_le())
    }

    fn pow(self, exp: u32) -> Self {
        Self(self.0.pow(exp))
    }
}
