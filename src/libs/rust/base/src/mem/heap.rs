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

use crate::arch::cfg;
use crate::io;
use crate::mem;

const HEAP_USED_BITS: usize = 0x5 << (8 * mem::size_of::<usize>() - 3);

#[repr(C, packed)]
pub struct HeapArea {
    pub next: usize, /* HEAP_USED_BITS set = used */
    pub prev: usize,
    _pad: [u8; 64 - mem::size_of::<usize>() * 2],
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

    fn heap_append(pages: usize);

    fn heap_free_memory() -> usize;
    fn heap_used_end() -> usize;
}

extern "C" {
    static _bss_end: u8;
    static mut heap_begin: *const HeapArea;
    static mut heap_end: *const HeapArea;
}

#[cfg(not(target_vendor = "host"))]
fn heap_bounds() -> (usize, usize) {
    use crate::arch;
    use crate::kif::TileDesc;
    use crate::math;

    unsafe {
        let begin = math::round_up(&_bss_end as *const u8 as usize, cfg::PAGE_SIZE);

        let env = arch::envdata::get();
        let tile_desc = TileDesc::new_from(env.tile_desc);
        let end = if tile_desc.has_mem() {
            tile_desc.stack_space().0
        }
        else {
            assert!(env.heap_size != 0);
            begin + env.heap_size as usize
        };

        (begin, end)
    }
}

#[cfg(target_vendor = "host")]
fn heap_bounds() -> (usize, usize) {
    use crate::arch::envdata;

    (
        envdata::heap_start(),
        envdata::heap_start() + cfg::APP_HEAP_SIZE,
    )
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
            llog!(
                DEF,
                "  Area[addr={:#x}, prev={:#x}, size={:#x}, used={}]",
                a as usize + mem::size_of::<HeapArea>(),
                (*a).backwards((*a).prev as usize) as usize + mem::size_of::<HeapArea>(),
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
    llog!(HEAP, "alloc({}) -> {:?}", size, p);
}

extern "C" fn heap_free_callback(p: *const u8) {
    llog!(HEAP, "free({:?})", p);
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
