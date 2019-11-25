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

//! Contains utilities

use core::intrinsics;
use core::slice;
use libc;
use num_traits::PrimInt;

/// Computes the square root of `n`.
///
/// Source: [Wikipedia](https://en.wikipedia.org/wiki/Methods_of_computing_square_roots)
pub fn sqrt(n: f32) -> f32 {
    let mut val_int: u32 = unsafe { intrinsics::transmute(n) };

    val_int = val_int.wrapping_sub(1 << 23); /* Subtract 2^m. */
    val_int >>= 1; /* Divide by 2. */
    val_int = val_int.wrapping_add(1 << 29); /* Add ((b + 1) / 2) * 2^m. */

    f32::from_bits(val_int)
}

/// Returns the size of `T`
pub const fn size_of<T>() -> usize {
    intrinsics::size_of::<T>()
}

/// Returns the size of `val`
pub fn size_of_val<T: ?Sized>(val: &T) -> usize {
    intrinsics::size_of_val(val)
}

/// Converts the given C string into a string slice
///
/// # Safety
///
/// This function assumes that `s` points to a permanently valid and null-terminated C string
pub unsafe fn cstr_to_str(s: *const i8) -> &'static str {
    let len = libc::strlen(s);
    let sl = slice::from_raw_parts(s, len as usize + 1);
    &*(&sl[..sl.len() - 1] as *const [i8] as *const str)
}

/// Creates a slice of `T`s for the given address range
///
/// # Safety
///
/// This function assumes that `start` points to a permanently valid array of `size` bytes
/// containing `T`s
pub unsafe fn slice_for<T>(start: *const T, size: usize) -> &'static [T] {
    slice::from_raw_parts(start, size)
}

/// Creates a mutable slice of `T`s for the given address range
///
/// # Safety
///
/// This function assumes that `start` points to a permanently valid and writable array of `size`
/// bytes containing `T`s
pub unsafe fn slice_for_mut<T>(start: *mut T, size: usize) -> &'static mut [T] {
    slice::from_raw_parts_mut(start, size)
}

/// Creates a byte slice for the given object
pub fn object_to_bytes<T: Sized>(obj: &T) -> &[u8] {
    let p: *const T = obj;
    let p: *const u8 = p as *const u8;
    unsafe { slice::from_raw_parts(p, size_of::<T>()) }
}

/// Creates a mutable byte slice for the given object
pub fn object_to_bytes_mut<T: Sized>(obj: &mut T) -> &mut [u8] {
    let p: *mut T = obj;
    let p: *mut u8 = p as *mut u8;
    unsafe { slice::from_raw_parts_mut(p, size_of::<T>()) }
}

fn _next_log2(size: usize, shift: u32) -> u32 {
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
/// assert_eq!(util::next_log2(4), 4);
/// assert_eq!(util::next_log2(5), 8);
/// ```
pub fn next_log2(size: usize) -> u32 {
    _next_log2(size, (size_of::<usize>() * 8 - 2) as u32)
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

/// Returns the minimum of `a` and `b`
pub fn min<T: Ord>(a: T, b: T) -> T {
    if a > b {
        b
    }
    else {
        a
    }
}

/// Returns the maximum of `a` and `b`
pub fn max<T: Ord>(a: T, b: T) -> T {
    if a > b {
        a
    }
    else {
        b
    }
}

/// Expands to the current function name.
#[macro_export]
macro_rules! function {
    () => {{
        fn f() {
        }
        fn type_name_of<T>(_: T) -> &'static str {
            extern crate core;
            unsafe { core::intrinsics::type_name::<T>() }
        }
        let name = type_name_of(f);
        &name[0..name.len() - 3]
    }};
}

/// Creates an struct where the members can be used as integers, similar to C enums.
///
/// # Examples
///
/// ```
/// int_enum! {
///     /// My enum
///     pub struct Test : u8 {
///        const VAL_1 = 0x0;
///        const VAL_2 = 0x1;
///     }
/// }
/// ```
///
/// Each struct member has the field `val`, which corresponds to its value. The macro implements the
/// traits `Debug`, `Display`, `Marshallable`, and `Unmarshallable`. Furthermore, it allows to
/// convert from the underlying type (here `u8`) to the struct.
#[macro_export]
macro_rules! int_enum {
    (
        $(#[$outer:meta])*
        pub struct $Name:ident: $T:ty {
            $(
                $(#[$inner:ident $($args:tt)*])*
                const $Flag:ident = $value:expr;
            )+
        }
    ) => (
        $(#[$outer])*
        #[derive(Copy, PartialEq, Eq, Clone, PartialOrd, Ord)]
        pub struct $Name {
            pub val: $T,
        }

        int_enum! {
            @enum_impl struct $Name : $T {
                $(
                    $(#[$inner $($args)*])*
                    const $Flag = $value;
                )+
            }
        }
    );

    (
        $(#[$outer:meta])*
        struct $Name:ident: $T:ty {
            $(
                $(#[$inner:ident $($args:tt)*])*
                const $Flag:ident = $value:expr;
            )+
        }
    ) => (
        $(#[$outer])*
        #[derive(Copy, PartialEq, Eq, Clone, PartialOrd, Ord)]
        struct $Name {
            pub val: $T,
        }

        int_enum! {
            @enum_impl struct $Name : $T {
                $(
                    const $Flag = $value;
                )+
            }
        }
    );

    (
        @enum_impl struct $Name:ident: $T:ty {
            $(
                $(#[$attr:ident $($args:tt)*])*
                const $Flag:ident = $value:expr;
            )+
        }
    ) => (
        impl $Name {
            $(
                $(#[$attr $($args)*])*
                #[allow(dead_code)]
                pub const $Flag: $Name = $Name { val: $value };
            )+

            pub fn print(&self, f: &mut $crate::_core::fmt::Formatter) -> $crate::_core::fmt::Result {
                $(
                    if self.val == $value {
                        return f.write_str(stringify!($Flag));
                    }
                )+
                f.write_str("(unknown)")
            }
        }

        impl $crate::serialize::Marshallable for $Name {
            fn marshall(&self, s: &mut dyn $crate::serialize::Sink) {
                s.push_word(self.val as u64);
            }
        }

        impl $crate::serialize::Unmarshallable for $Name {
            fn unmarshall(s: &mut dyn $crate::serialize::Source) -> Self {
                let val = s.pop_word() as $T;
                $Name { val: val }
            }
        }

        impl From<$T> for $Name {
            fn from(val: $T) -> Self {
                $Name { val: val }
            }
        }

        impl $crate::_core::fmt::Debug for $Name {
            fn fmt(&self, f: &mut $crate::_core::fmt::Formatter) -> $crate::_core::fmt::Result {
                write!(f, "{}:", self.val)?;
                self.print(f)
            }
        }
        impl $crate::_core::fmt::Display for $Name {
            fn fmt(&self, f: &mut $crate::_core::fmt::Formatter) -> $crate::_core::fmt::Result {
                self.print(f)
            }
        }
    )
}
