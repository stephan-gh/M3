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

//! Contains utilities

use core::slice;

use crate::libc;
use crate::mem;

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
    unsafe { slice::from_raw_parts(p, mem::size_of::<T>()) }
}

/// Creates a mutable byte slice for the given object
pub fn object_to_bytes_mut<T: Sized>(obj: &mut T) -> &mut [u8] {
    let p: *mut T = obj;
    let p: *mut u8 = p as *mut u8;
    unsafe { slice::from_raw_parts_mut(p, mem::size_of::<T>()) }
}

/// Expands to the current function name.
#[macro_export]
macro_rules! function {
    () => {{
        fn f() {
        }
        fn type_name_of<T>(_: T) -> &'static str {
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
/// traits [`Debug`](core::fmt::Debug), [`Display`](core::fmt::Display),
/// [`Marshallable`](crate::serialize::Marshallable), and
/// [`Unmarshallable`](crate::serialize::Unmarshallable). Furthermore, it allows to convert from the
/// underlying type (here [`u8`]) to the struct.
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

            pub fn print(&self, f: &mut $crate::_core::fmt::Formatter<'_>) -> $crate::_core::fmt::Result {
                $(
                    if self.val == $value {
                        return f.write_str(stringify!($Flag));
                    }
                )+
                f.write_str("(unknown)")
            }
        }

        impl $crate::serialize::Marshallable for $Name {
            fn marshall(&self, s: &mut $crate::serialize::Sink<'_>) {
                s.push_word(self.val as u64);
            }
        }

        impl $crate::serialize::Unmarshallable for $Name {
            fn unmarshall(s: &mut $crate::serialize::Source<'_>) -> Result<Self, $crate::errors::Error> {
                let val = s.pop_word()? as $T;
                Ok($Name { val })
            }
        }

        impl From<$T> for $Name {
            fn from(val: $T) -> Self {
                $Name { val }
            }
        }

        impl $crate::_core::fmt::Debug for $Name {
            fn fmt(&self, f: &mut $crate::_core::fmt::Formatter<'_>) -> $crate::_core::fmt::Result {
                write!(f, "{}:", self.val)?;
                self.print(f)
            }
        }
        impl $crate::_core::fmt::Display for $Name {
            fn fmt(&self, f: &mut $crate::_core::fmt::Formatter<'_>) -> $crate::_core::fmt::Result {
                self.print(f)
            }
        }
    )
}
