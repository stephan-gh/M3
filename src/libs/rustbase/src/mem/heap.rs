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

//! Contains the malloc implementation

use arch::cfg;
use core::intrinsics;
use io;
use libc;
use util;

const HEAP_USED_BITS: usize = 0x5 << (8 * util::size_of::<usize>() - 3);

#[repr(C, packed)]
pub struct HeapArea {
    pub next: usize, /* HEAP_USED_BITS set = used */
    pub prev: usize,
    _pad: [u8; 64 - util::size_of::<usize>() * 2],
}

impl HeapArea {
    fn is_used(&self) -> bool {
        (self.next & HEAP_USED_BITS) != 0
    }

    unsafe fn forward(&self, size: usize) -> *const HeapArea {
        let next = (self as *const _ as usize) + size;
        next as *const HeapArea
    }

    unsafe fn backwards(&self, size: usize) -> *const HeapArea {
        let prev = (self as *const _ as usize) - size;
        prev as *const HeapArea
    }
}

extern "C" {
    fn heap_set_alloc_callback(cb: extern "C" fn(p: *const u8, size: usize));
    fn heap_set_free_callback(cb: extern "C" fn(p: *const u8));
    fn heap_set_oom_callback(cb: extern "C" fn(size: usize) -> bool);
    fn heap_set_dblfree_callback(cb: extern "C" fn(p: *const u8));

    /// Initializes the heap
    fn heap_init(begin: usize, end: usize);

    /// Allocates `size` bytes on the heap
    fn heap_alloc(size: usize) -> *mut libc::c_void;

    /// Allocates `n * size` on the heap and initializes it to 0
    fn heap_calloc(n: usize, size: usize) -> *mut libc::c_void;

    /// Reallocates `n` to be `size` bytes large
    ///
    /// This implementation might increase the size of the area or shink it. It might also free the
    /// current area and allocate a new area of `size` bytes.
    fn heap_realloc(p: *mut libc::c_void, size: usize) -> *mut libc::c_void;

    /// Frees the area at `p`
    fn heap_free(p: *mut libc::c_void);

    fn heap_append(pages: usize);

    fn heap_free_memory() -> usize;
    fn heap_used_end() -> usize;
}

extern "C" {
    static _bss_end: u8;
    static mut heap_begin: *const HeapArea;
    static mut heap_end: *const HeapArea;
}

#[cfg(target_os = "none")]
fn heap_bounds() -> (usize, usize) {
    use arch;
    use kif::PEDesc;

    unsafe {
        let begin = util::round_up(&_bss_end as *const u8 as usize, util::size_of::<HeapArea>());

        let env = arch::envdata::get();
        let end = if env.heap_size == 0 {
            PEDesc::new_from(env.pe_desc).mem_size() - cfg::RECVBUF_SIZE_SPM
        }
        else if PEDesc::new_from(env.pe_desc).has_mmu() && env.pe_id == 0 {
            util::round_up(begin as usize, cfg::PAGE_SIZE) + (4096 + 2048) * 1024
        }
        else {
            util::round_up(begin as usize, cfg::PAGE_SIZE) + env.heap_size as usize
        };

        (begin, end)
    }
}

#[cfg(target_os = "linux")]
fn heap_bounds() -> (usize, usize) {
    use arch::envdata;

    (
        envdata::heap_start(),
        envdata::heap_start() + cfg::APP_HEAP_SIZE,
    )
}

/// Allocates `size` bytes from the heap.
pub fn alloc(size: usize) -> *mut libc::c_void {
    unsafe { heap_alloc(size) }
}

pub fn init() {
    let (begin, end) = heap_bounds();

    unsafe {
        heap_init(begin, end);

        if io::log::HEAP {
            heap_set_alloc_callback(heap_alloc_callback);
            heap_set_free_callback(heap_free_callback);
        }
        heap_set_dblfree_callback(heap_dblfree_callback);
        heap_set_oom_callback(heap_oom_callback);
    }
}

/// Appends the given number of pages as free memory to the heap.
pub fn append(pages: usize) {
    unsafe {
        heap_append(pages);
    }
}

/// Returns the number of free bytes on the heap.
pub fn free_memory() -> usize {
    unsafe { heap_free_memory() }
}

/// Returns the end of used part of the heap.
pub fn end() -> usize {
    unsafe { heap_end as usize }
}

/// Returns the end of used part of the heap.
pub fn used_end() -> usize {
    unsafe { heap_used_end() }
}

/// Prints the heap.
pub fn print() {
    unsafe {
        let print_area = |a: *const HeapArea| {
            log!(
                DEF,
                "  Area[addr={:#x}, prev={:#x}, size={:#x}, used={}]",
                a as usize + util::size_of::<HeapArea>(),
                (*a).backwards((*a).prev as usize) as usize + util::size_of::<HeapArea>(),
                (*a).next & !HEAP_USED_BITS,
                (*a).is_used()
            );
        };

        let mut a = heap_begin;
        while a < heap_end {
            print_area(a);
            a = (*a).forward(((*a).next & !HEAP_USED_BITS) as usize);
        }
        print_area(heap_end);
    }
}

extern "C" fn heap_alloc_callback(p: *const u8, size: usize) {
    log!(HEAP, "alloc({}) -> {:?}", size, p);
}

extern "C" fn heap_free_callback(p: *const u8) {
    log!(HEAP, "free({:?})", p);
}

extern "C" fn heap_dblfree_callback(p: *const u8) {
    panic!("Used bits not set for {:?}; double free?", p);
}

extern "C" fn heap_oom_callback(size: usize) -> bool {
    panic!(
        "Unable to allocate {} bytes on the heap: out of memory",
        size
    );
}

#[no_mangle]
extern "C" fn __rdl_alloc(size: usize, _align: usize, _err: *mut u8) -> *mut libc::c_void {
    alloc(size)
}

#[no_mangle]
extern "C" fn __rdl_dealloc(ptr: *mut libc::c_void, _size: usize, _align: usize) {
    unsafe { heap_free(ptr) };
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
    unsafe { heap_realloc(ptr, new_size) }
}

#[no_mangle]
extern "C" fn __rdl_alloc_zeroed(size: usize, _align: usize, _err: *mut u8) -> *mut libc::c_void {
    unsafe { heap_calloc(size, 1) }
}

#[no_mangle]
extern "C" fn __rdl_oom(_err: *const u8) -> ! {
    unsafe { intrinsics::abort() };
}

#[no_mangle]
extern "C" fn __rdl_usable_size(_layout: *const u8, _min: *mut usize, _max: *mut usize) {
    // TODO implement me
}

#[no_mangle]
extern "C" fn __rdl_alloc_excess(
    size: usize,
    _align: usize,
    _excess: *mut usize,
    _err: *mut u8,
) -> *mut libc::c_void {
    // TODO is that correct?
    alloc(size)
}

#[no_mangle]
extern "C" fn __rdl_realloc_excess(
    ptr: *mut libc::c_void,
    _old_size: usize,
    _old_align: usize,
    new_size: usize,
    _new_align: usize,
    _excess: *mut usize,
    _err: *mut u8,
) -> *mut libc::c_void {
    // TODO is that correct?
    unsafe { heap_realloc(ptr, new_size) }
}

#[no_mangle]
extern "C" fn __rdl_grow_in_place(
    _ptr: *mut libc::c_void,
    _old_size: usize,
    _old_align: usize,
    _new_size: usize,
    _new_align: usize,
) -> u8 {
    // TODO implement me
    0
}

#[no_mangle]
extern "C" fn __rdl_shrink_in_place(
    _ptr: *mut libc::c_void,
    _old_size: usize,
    _old_align: usize,
    _new_size: usize,
    _new_align: usize,
) -> u8 {
    // TODO implement me
    0
}
