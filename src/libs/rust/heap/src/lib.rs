/*
 * Copyright (C) 2020-2021 Nils Asmussen, Barkhausen Institut
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

#![no_std]

use base::libc;

extern "C" {
    /// Allocates `size` bytes on the heap
    fn malloc(size: usize) -> *mut libc::c_void;

    /// Allocates `n * size` on the heap and initializes it to 0
    fn calloc(n: usize, size: usize) -> *mut libc::c_void;

    /// Reallocates `n` to be `size` bytes large
    ///
    /// This implementation might increase the size of the area or shink it. It might also free the
    /// current area and allocate a new area of `size` bytes.
    fn realloc(p: *mut libc::c_void, size: usize) -> *mut libc::c_void;

    /// Frees the area at `p`
    fn free(p: *mut libc::c_void);
}

#[no_mangle]
extern "C" fn __rdl_alloc(size: usize, _align: usize, _err: *mut u8) -> *mut libc::c_void {
    unsafe { malloc(size) }
}

#[no_mangle]
extern "C" fn __rdl_dealloc(ptr: *mut libc::c_void, _size: usize, _align: usize) {
    unsafe { free(ptr) };
}

#[no_mangle]
extern "C" fn __rdl_realloc(
    ptr: *mut libc::c_void,
    _old_size: usize,
    _old_align: usize,
    new_size: usize,
    _new_align: usize,
    _err: *mut u8,
) -> *mut libc::c_void {
    unsafe { realloc(ptr, new_size) }
}

#[no_mangle]
extern "C" fn __rdl_alloc_zeroed(size: usize, _align: usize, _err: *mut u8) -> *mut libc::c_void {
    unsafe { calloc(size, 1) }
}
