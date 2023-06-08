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

#![feature(register_tool)]
#![register_tool(m3_async)]
#![no_std]

mod args;
mod cap;
mod com;
mod ktcu;
mod mem;
mod platform;
mod runtime;
mod slab;
mod syscalls;
mod tiles;

use base::cfg;
use base::env;
use base::io::{self, LogFlags};
use base::kif::TileDesc;
use base::log;
use base::machine;
use base::mem::VirtAddr;
use base::tcu;
use base::util::math;

use core::ptr;

use crate::tiles::ActivityMng;

extern "C" {
    static _bss_end: u8;

    fn __m3_init_libc(argc: i32, argv: *const *const u8, envp: *const *const u8, tls: bool);
    fn __m3_heap_get_end() -> usize;
    fn __m3_heap_set_area(begin: usize, end: usize);
    fn __m3_heap_append(pages: usize);
}

#[no_mangle]
pub extern "C" fn abort() -> ! {
    exit(1);
}

#[no_mangle]
pub extern "C" fn exit(_code: i32) -> ! {
    log!(LogFlags::Info, "Shutting down");
    machine::write_coverage(0);
    machine::shutdown();
}

fn create_rbufs() {
    let sysc_slot_size = 9;
    let sysc_rbuf_size = math::next_log2(cfg::MAX_ACTS) + sysc_slot_size;
    let serv_slot_size = 8;
    let serv_rbuf_size = math::next_log2(crate::com::MAX_PENDING_MSGS) + serv_slot_size;
    let tm_slot_size = 7;
    let tm_rbuf_size = math::next_log2(cfg::MAX_ACTS) + tm_slot_size;
    let total_size = (1 << sysc_rbuf_size) + (1 << serv_rbuf_size) + (1 << tm_rbuf_size);

    let tiledesc = TileDesc::new_from(env::boot().tile_desc);
    let mut rbuf = if tiledesc.has_virtmem() {
        // we need to make sure that receive buffers are physically contiguous. thus, allocate a new
        // chunk of physical memory and map it somewhere.
        let total_size = math::round_up(total_size, cfg::PAGE_SIZE);
        let rbuf = cfg::RBUF_STD_ADDR;
        runtime::paging::map_new_mem(rbuf, total_size / cfg::PAGE_SIZE, cfg::PAGE_SIZE);
        rbuf
    }
    else {
        tiledesc.rbuf_space().0
    };

    // TODO add second syscall REP
    ktcu::recv_msgs(ktcu::KSYS_EP, rbuf, sysc_rbuf_size, sysc_slot_size)
        .expect("Unable to config syscall REP");
    rbuf += 1 << sysc_rbuf_size as usize;

    ktcu::recv_msgs(ktcu::KSRV_EP, rbuf, serv_rbuf_size, serv_slot_size)
        .expect("Unable to config service REP");
    rbuf += 1 << serv_rbuf_size as usize;

    ktcu::recv_msgs(ktcu::KPEX_EP, rbuf, tm_rbuf_size, tm_slot_size)
        .expect("Unable to config tilemux REP");
}

fn create_heap() {
    unsafe {
        let heap_start = math::round_up(&_bss_end as *const _ as usize, cfg::PAGE_SIZE);
        let mut heap_end = __m3_heap_get_end();
        assert_eq!(heap_end, 0);
        let desc = TileDesc::new_from(env::boot().tile_desc);
        if desc.has_virtmem() {
            heap_end = heap_start + 128 * cfg::PAGE_SIZE;
        }
        else {
            heap_end = desc.stack_space().0.as_local();
        }
        __m3_heap_set_area(heap_start, heap_end);
    }
}

fn extend_heap() {
    if platform::tile_desc(platform::kernel_tile()).has_virtmem() {
        let free_contiguous = mem::borrow_mut().largest_contiguous(mem::MemType::KERNEL);
        if let Some(bytes) = free_contiguous {
            let heap_end = VirtAddr::from(unsafe { __m3_heap_get_end() });

            // determine page count and virtual start address
            let pages = (bytes as usize) >> cfg::PAGE_BITS;
            let virt = math::round_up(heap_end, VirtAddr::from(cfg::PAGE_SIZE));

            // first map small pages until the next large page
            let virt_next_lpage =
                (virt + cfg::LPAGE_SIZE - 1) & VirtAddr::from(!(cfg::LPAGE_SIZE - 1));
            let small_pages = ((virt_next_lpage - virt) >> cfg::PAGE_BITS).as_local();

            runtime::paging::map_new_mem(virt, small_pages, cfg::PAGE_SIZE);
            unsafe { __m3_heap_append(small_pages) };

            // now map the rest with large pages
            let large_pages = ((pages - small_pages) * cfg::PAGE_SIZE) / cfg::LPAGE_SIZE;
            let pages_per_lpage = cfg::LPAGE_SIZE / cfg::PAGE_SIZE;
            runtime::paging::map_new_mem(
                virt_next_lpage,
                large_pages * pages_per_lpage,
                cfg::LPAGE_SIZE,
            );
            unsafe { __m3_heap_append(large_pages * pages_per_lpage) };
        }
    }
}

#[no_mangle]
pub extern "C" fn env_run() {
    unsafe { __m3_init_libc(0, ptr::null(), ptr::null(), false) };
    create_heap();
    crate::slab::init();
    io::init(
        tcu::TileId::new_from_raw(env::boot().tile_id as u16),
        "kernel",
    );

    runtime::paging::init();
    runtime::exceptions::init();
    crate::com::init_queues();

    log!(LogFlags::Info, "Entered raw mode; Quit via Ctrl+]");

    args::parse();

    platform::init();
    create_rbufs();
    extend_heap();
    thread::init();
    tiles::init();

    log!(LogFlags::Info, "Kernel is ready!");

    workloop();
}

pub fn thread_startup() {
    workloop();
}

fn workloop() -> ! {
    if thread::cur().is_main() {
        ActivityMng::start_root_async().expect("starting root failed");
    }

    while ActivityMng::count() > 0 {
        if env::boot().platform != env::Platform::Hw {
            tcu::TCU::sleep().unwrap();
        }

        if let Some(msg) = ktcu::fetch_msg(ktcu::KSYS_EP) {
            syscalls::handle_async(msg);
        }

        if let Some(msg) = ktcu::fetch_msg(ktcu::KSRV_EP) {
            unsafe {
                let squeue: *mut com::SendQueue = msg.header.label() as *mut _;
                (*squeue).received_reply(msg);
            }
        }

        if let Some(msg) = ktcu::fetch_msg(ktcu::KPEX_EP) {
            let tile = tcu::TileId::new_from_raw(msg.header.label() as u16);
            crate::tiles::TileMux::handle_call_async(crate::tiles::tilemng::tilemux(tile), msg);
        }

        thread::try_yield();
    }

    // do the tile deinit just once
    if tiles::tilemng::state() == tiles::tilemng::State::RUNNING {
        // with all activities gone, we should only have the main thread left; add another thread for
        // the asynchronous tile reset
        assert_eq!(thread::thread_count(), 0);
        thread::add_thread(VirtAddr::from(thread_startup as *const ()), 0);

        // trigger the shutdown of tiles
        tiles::deinit_async();
    }

    // wait until all tiles are shut down
    while tiles::tilemng::state() != tiles::tilemng::State::SHUTDOWN {
        tcu::TCU::sleep().ok();

        if let Some(msg) = ktcu::fetch_msg(ktcu::KSRV_EP) {
            unsafe {
                let squeue: *mut com::SendQueue = msg.header.label() as *mut _;
                (*squeue).received_reply(msg);
            }
        }

        thread::try_yield();
    }

    // if we get back here, all activities and multiplexers on user tiles are shut down, so we can
    // shut down the kernel tile as well
    exit(0);
}
