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

pub const PAGE_BITS: usize          = 12;
pub const PAGE_SIZE: usize          = 1 << PAGE_BITS;
pub const PAGE_MASK: usize          = PAGE_SIZE - 1;

pub const LPAGE_BITS: usize         = 21;
pub const LPAGE_SIZE: usize         = 1 << LPAGE_BITS;
pub const LPAGE_MASK: usize         = LPAGE_SIZE - 1;

pub const PE_INFO_ADDR: usize       = 0x3FBF_F000;
pub const PE_INFO_SIZE: usize       = PAGE_SIZE;

pub const RECVBUF_SPACE: usize      = 0x3FC0_0000;
pub const RECVBUF_SIZE: usize       = 4 * PAGE_SIZE;
pub const RECVBUF_SIZE_SPM: usize   = 16384;
pub const MAX_RB_SIZE: usize        = 32;

pub const PEMUX_RBUF_SPACE: usize   = 0x1FF000;
pub const PEMUX_RBUF_SIZE: usize    = 1 * PAGE_SIZE;

pub const PE_MEM_BASE: usize        = 0xE000_0000;

pub const MEM_CAP_END: usize        = RECVBUF_SPACE;

pub const ENV_START: usize          = 0x0020_0000;
pub const ENV_SIZE: usize           = 0x2000;
pub const STACK_SIZE: usize         = 0xF000;
pub const STACK_BOTTOM: usize       = ENV_START + ENV_SIZE + PAGE_SIZE;
pub const STACK_TOP: usize          = STACK_BOTTOM + STACK_SIZE;

pub const APP_HEAP_SIZE: usize      = 64 * 1024 * 1024;
pub const MOD_HEAP_SIZE: usize      = 4 * 1024 * 1024;

pub const KPEX_RBUF_ORD: i32        = 6;
pub const PEXUP_RBUF_ORD: i32       = 6;
pub const SYSC_RBUF_ORD: i32        = 9;
pub const UPCALL_RBUF_ORD: i32      = 6;
pub const DEF_RBUF_ORD: i32         = 8;
pub const VMA_RBUF_ORD: i32         = 6;

pub const KPEX_RBUF_SIZE: usize     = 1 << KPEX_RBUF_ORD;
pub const PEXUP_RBUF_SIZE: usize    = 1 << PEXUP_RBUF_ORD;
pub const SYSC_RBUF_SIZE: usize     = 1 << SYSC_RBUF_ORD;
pub const UPCALL_RBUF_SIZE: usize   = 1 << UPCALL_RBUF_ORD;
pub const DEF_RBUF_SIZE: usize      = 1 << DEF_RBUF_ORD;
pub const VMA_RBUF_SIZE: usize      = 1 << VMA_RBUF_ORD;
