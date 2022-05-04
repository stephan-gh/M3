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

pub const PAGE_BITS: usize = 12;
pub const PAGE_SIZE: usize = 1 << PAGE_BITS;
pub const PAGE_MASK: usize = PAGE_SIZE - 1;
pub const LPAGE_SIZE: usize = 1 << 21;

pub const MAX_RB_SIZE: usize = usize::max_value();

pub const MEM_CAP_END: usize = 0xFFFF_FFFF_FFFF_FFFF;

pub const TILE_COUNT: usize = 18;
pub const MAX_ACTS: usize = TILE_COUNT - 1;

pub const TOTAL_MEM_SIZE: usize = 2048 * 1024 * 1024;
pub const FS_MAX_SIZE: usize = 640 * 1024 * 1024;
pub const STACK_SIZE: usize = 0x8000;

pub const RBUF_STD_ADDR: usize = 0;
pub const RBUF_STD_SIZE: usize = PAGE_SIZE;
pub const RBUF_ADDR: usize = RBUF_STD_ADDR + RBUF_STD_SIZE;
pub const RBUF_SIZE: usize = 64 * 1024;
pub const RBUF_SIZE_SPM: usize = 64 * 1024;

pub const LOCAL_MEM_SIZE: usize = 4 * 1024 * 1024;
pub const EPMEM_SIZE: usize = 1 * 1024 * 1024;

pub const FIXED_KMEM: usize = 2 * 1024 * 1024;
pub const FIXED_ROOT_MEM: usize = 128 * 1024 * 1024;

pub const SERIAL_BUF_ORD: u32 = 6;

pub const KPEX_RBUF_ORD: i32 = 6;
pub const SYSC_RBUF_ORD: i32 = 9;
pub const UPCALL_RBUF_ORD: i32 = 9;
pub const DEF_RBUF_ORD: i32 = 8;

pub const KPEX_RBUF_SIZE: usize = 1 << KPEX_RBUF_ORD;
pub const SYSC_RBUF_SIZE: usize = 1 << SYSC_RBUF_ORD;
pub const UPCALL_RBUF_SIZE: usize = 1 << UPCALL_RBUF_ORD;
pub const DEF_RBUF_SIZE: usize = 1 << DEF_RBUF_ORD;
pub const TMUP_RBUF_SIZE: usize = 0;
