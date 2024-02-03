/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
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

//! Contains memory management abstractions

mod buffer;
mod globaddr;
mod map;
mod physaddr;
mod virtaddr;

pub use self::buffer::{AlignedBuf, MsgBuf, MsgBufRef};
pub use self::globaddr::{GlobAddr, GlobAddrRaw, GlobOff};
pub use self::map::MemMap;
pub use self::physaddr::{PhysAddr, PhysAddrRaw};
pub use self::virtaddr::{VirtAddr, VirtAddrRaw};
pub use core::mem::{
    align_of, align_of_val, forget, offset_of, replace, size_of, size_of_val, MaybeUninit,
};

#[macro_export]
macro_rules! impl_prim_int {
    ($T:ty, $R:ty) => {
        impl num_traits::Saturating for $T {
            fn saturating_add(self, v: Self) -> Self {
                Self(self.0.saturating_add(v.0))
            }

            fn saturating_sub(self, v: Self) -> Self {
                Self(self.0.saturating_sub(v.0))
            }
        }

        impl core::ops::Div for $T {
            type Output = Self;

            fn div(self, rhs: Self) -> Self::Output {
                Self(self.0 / rhs.0)
            }
        }

        impl num_traits::CheckedDiv for $T {
            fn checked_div(&self, v: &Self) -> Option<Self> {
                self.0.checked_div(v.0).map(|v| Self(v))
            }
        }

        impl core::ops::Rem for $T {
            type Output = Self;

            fn rem(self, rhs: Self) -> Self::Output {
                Self(self.0 % rhs.0)
            }
        }

        impl core::ops::Mul for $T {
            type Output = Self;

            fn mul(self, rhs: Self) -> Self::Output {
                Self(self.0 * rhs.0)
            }
        }

        impl num_traits::CheckedMul for $T {
            fn checked_mul(&self, v: &Self) -> Option<Self> {
                self.0.checked_mul(v.0).map(|v| Self(v))
            }
        }

        impl core::ops::Add<$T> for $T {
            type Output = Self;

            fn add(self, rhs: Self) -> Self::Output {
                Self(self.0 + rhs.0)
            }
        }

        impl num_traits::CheckedAdd for $T {
            fn checked_add(&self, v: &Self) -> Option<Self> {
                self.0.checked_add(v.0).map(|v| Self(v))
            }
        }

        impl core::ops::Sub<$T> for $T {
            type Output = Self;

            fn sub(self, rhs: Self) -> Self::Output {
                Self(self.0 - rhs.0)
            }
        }

        impl num_traits::CheckedSub for $T {
            fn checked_sub(&self, v: &Self) -> Option<Self> {
                self.0.checked_sub(v.0).map(|v| Self(v))
            }
        }

        impl core::ops::Shr<usize> for $T {
            type Output = Self;

            fn shr(self, rhs: usize) -> Self::Output {
                Self(self.0 >> rhs)
            }
        }

        impl core::ops::Shl<usize> for $T {
            type Output = Self;

            fn shl(self, rhs: usize) -> Self::Output {
                Self(self.0 << rhs)
            }
        }

        impl core::ops::BitXor<Self> for $T {
            type Output = Self;

            fn bitxor(self, rhs: Self) -> Self::Output {
                Self(self.0 ^ rhs.0)
            }
        }

        impl core::ops::BitOr<Self> for $T {
            type Output = Self;

            fn bitor(self, rhs: Self) -> Self::Output {
                Self(self.0 | rhs.0)
            }
        }

        impl core::ops::BitAnd<Self> for $T {
            type Output = Self;

            fn bitand(self, rhs: Self) -> Self::Output {
                Self(self.0 & rhs.0)
            }
        }

        impl core::ops::Not for $T {
            type Output = Self;

            fn not(self) -> Self::Output {
                Self(!self.0)
            }
        }

        impl num_traits::Bounded for $T {
            fn min_value() -> Self {
                Self(<$R>::min_value())
            }

            fn max_value() -> Self {
                Self(<$R>::max_value())
            }
        }

        impl num_traits::ToPrimitive for $T {
            fn to_i64(&self) -> Option<i64> {
                self.0.to_i64()
            }

            fn to_u64(&self) -> Option<u64> {
                self.0.to_u64()
            }
        }

        impl num_traits::NumCast for $T {
            fn from<T: num_traits::ToPrimitive>(n: T) -> Option<Self> {
                <$R as num_traits::NumCast>::from(n).map(|v| Self(v))
            }
        }

        impl num_traits::Zero for $T {
            fn zero() -> Self {
                Self(0)
            }

            fn is_zero(&self) -> bool {
                self.as_raw() == 0
            }
        }

        impl num_traits::One for $T {
            fn one() -> Self {
                Self(1)
            }
        }

        impl num_traits::Num for $T {
            type FromStrRadixErr = core::num::ParseIntError;

            fn from_str_radix(str: &str, radix: u32) -> Result<Self, Self::FromStrRadixErr> {
                <$R>::from_str_radix(str, radix).map(|v| Self(v))
            }
        }

        impl num_traits::PrimInt for $T {
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
                Self(<$R as num_traits::PrimInt>::from_be(x.0))
            }

            fn from_le(x: Self) -> Self {
                Self(<$R as num_traits::PrimInt>::from_le(x.0))
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
    };
}
