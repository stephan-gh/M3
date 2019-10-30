/*
 * Copyright (C) 2016-2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#pragma once

#define FS_IMG_OFFSET       0x0

#define PAGE_BITS           12
#define PAGE_SIZE           (static_cast<size_t>(1) << PAGE_BITS)
#define PAGE_MASK           (PAGE_SIZE - 1)

#define FIXED_KMEM          (2 * 1024 * 1024)
#define VPE_EXTRA_MEM       (64 * 1024)

#define ROOT_HEAP_SIZE      (512 * 1024)
#define APP_HEAP_SIZE       (64 * 1024 * 1024)
#define EPMEM_SIZE          0

#define EP_COUNT            16
#define TOTAL_EPS           128 // TODO temporary

// Application memory layout:
// +----------------------------+ 0x0
// |      reserved for PTs      |
// +----------------------------+ 0x100000
// |         PEMUX_YIELD        |
// +----------------------------+ 0x100008
// |         PEMUX_FLAGS        |
// +----------------------------+ 0x100010
// |       PEMux code+data      |
// +----------------------------+ 0x200000
// |         environment        |
// +----------------------------+ 0x202000
// |         app stack          |
// +----------------------------+ 0x212000
// |       app code+data        |
// +----------------------------+ 0x3FC00000
// |        recv buffers        |
// +----------------------------+ 0x3FC04000
// |            ...             |
// +----------------------------+ 0xF0000000
// |          DTU MMIO          |
// +----------------------------+ 0xF0002000

#define PEMUX_YIELD         0x100000
#define PEMUX_FLAGS         0x100008

#define ENV_START           0x200000
#define ENV_SIZE            0x2000
#define ENV_END             (ENV_START + ENV_SIZE)

#define STACK_SIZE          0xF000
#define STACK_BOTTOM        (ENV_END + 0x1000)
#define STACK_TOP           (ENV_END + STACK_SIZE)

#define RECVBUF_SPACE       0x3FC00000
#define RECVBUF_SIZE        (4U * PAGE_SIZE)
#define RECVBUF_SIZE_SPM    16384U

#define MAX_RB_SIZE         32

#define KPEX_RBUF_ORDER     6
#define KPEX_RBUF_SIZE      (1 << KPEX_RBUF_ORDER)
#define KPEX_RBUF           RECVBUF_SPACE

#define PEXUP_RBUF_ORDER    6
#define PEXUP_RBUF_SIZE     (1 << PEXUP_RBUF_ORDER)
#define PEXUP_RBUF          (KPEX_RBUF + KPEX_RBUF_SIZE)

#define SYSC_RBUF_ORDER     9
#define SYSC_RBUF_SIZE      (1 << SYSC_RBUF_ORDER)
#define SYSC_RBUF           (PEXUP_RBUF + PEXUP_RBUF_SIZE)

#define UPCALL_RBUF_ORDER   6
#define UPCALL_RBUF_SIZE    (1 << UPCALL_RBUF_ORDER)
#define UPCALL_RBUF         (SYSC_RBUF + SYSC_RBUF_SIZE)

#define DEF_RBUF_ORDER      8
#define DEF_RBUF_SIZE       (1 << DEF_RBUF_ORDER)
#define DEF_RBUF            (UPCALL_RBUF + UPCALL_RBUF_SIZE)

#define VMA_RBUF_ORDER      6
#define VMA_RBUF_SIZE       (1 << VMA_RBUF_ORDER)
#define VMA_RBUF            (DEF_RBUF + DEF_RBUF_SIZE)

#define MEMCAP_END          RECVBUF_SPACE
