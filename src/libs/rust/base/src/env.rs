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

//! Provides access to the program environment

use core::iter;
use core::ops::FnOnce;

use crate::arch;
use crate::boxed::Box;
use crate::mem;
use crate::util;

/// The closure used by `Activity::run`
pub struct Closure {
    func: Option<Box<dyn FnOnce() -> i32 + Send>>,
}

impl Closure {
    /// Creates a new object for given closure
    pub fn new<F>(func: Box<F>) -> Self
    where
        F: FnOnce() -> i32 + Send + 'static,
    {
        Closure { func: Some(func) }
    }

    /// Calls the closure (can only be done once) and returns its exit code
    pub fn call(&mut self) -> i32 {
        match mem::replace(&mut self.func, None) {
            Some(c) => c(),
            None => 1,
        }
    }
}

/// The command line argument iterator
///
/// # Examples
///
/// ```
/// for arg in env::args() {
///     println!("{}", arg);
/// }
/// ```
#[derive(Copy, Clone)]
pub struct Args {
    pos: isize,
}

impl Args {
    fn arg(self, idx: isize) -> &'static str {
        // safety: we assume that our loader has put valid strings at argv
        unsafe {
            let args = arch::envdata::get().argv as *const u64;
            let arg = *args.offset(idx);
            util::cstr_to_str(arg as *const i8)
        }
    }

    /// Returns true if there are no arguments
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }

    /// Returns the number of arguments
    pub fn len(self) -> usize {
        arch::envdata::get().argc as usize
    }
}

impl iter::Iterator for Args {
    type Item = &'static str;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos < arch::envdata::get().argc as isize {
            let arg = self.arg(self.pos);
            self.pos += 1;
            Some(arg)
        }
        else {
            None
        }
    }
}

/// Returns the argument iterator
pub fn args() -> Args {
    Args { pos: 0 }
}
