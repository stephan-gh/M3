/*
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

//! Contains math functions

use num_traits::PrimInt;

use crate::mem;

/// Computes the square root of `val`.
///
/// Source: [Wikipedia](https://en.wikipedia.org/wiki/Methods_of_computing_square_roots)
pub fn sqrt(val: f32) -> f32 {
    let mut val_int: u32 = val.to_bits();

    val_int = val_int.wrapping_sub(1 << 23); /* Subtract 2^m. */
    val_int >>= 1; /* Divide by 2. */
    val_int = val_int.wrapping_add(1 << 29); /* Add ((b + 1) / 2) * 2^m. */

    f32::from_bits(val_int)
}

const fn _next_log2(size: usize, shift: u32) -> u32 {
    if size > (1 << shift) {
        shift + 1
    }
    else if shift == 0 {
        0
    }
    else {
        _next_log2(size, shift - 1)
    }
}

/// Returns the next power of 2
///
/// # Examples
///
/// ```
/// assert_eq!(util::next_log2(4), 2);
/// assert_eq!(util::next_log2(5), 3);
/// ```
pub const fn next_log2(size: usize) -> u32 {
    _next_log2(size, (mem::size_of::<usize>() * 8 - 2) as u32)
}

/// Rounds the given value up to the given alignment
///
/// # Examples
///
/// ```
/// assert_eq!(util::round_up(0x123, 0x1000), 0x1000);
/// ```
pub fn round_up<T: PrimInt>(value: T, align: T) -> T {
    (value + align - T::one()) & !(align - T::one())
}

/// Rounds the given value down to the given alignment
///
/// # Examples
///
/// ```
/// assert_eq!(util::round_dn(0x123, 0x1000), 0x0);
/// ```
pub fn round_dn<T: PrimInt>(value: T, align: T) -> T {
    value & !(align - T::one())
}

/// Returns true if `addr` is aligned to `align`
pub fn is_aligned<T: PrimInt>(addr: T, align: T) -> bool {
    (addr & (align - T::one())) == T::zero()
}

/// Assuming that `startx` < `endx` and `endx` is not included (that means with start=0 and end=10
/// 0 .. 9 is used), the function determines whether the two ranges overlap anywhere.
pub fn overlaps<T: Ord>(start1: T, end1: T, start2: T, end2: T) -> bool {
    (start1 >= start2 && start1 < end2) // start in range
    || (end1 > start2 && end1 <= end2)  // end in range
    || (start1 < start2 && end1 > end2) // complete overlapped
}
