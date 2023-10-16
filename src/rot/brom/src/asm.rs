/*
 * Copyright (C) 2023-2024, Stephan Gerhold <stephan@gerhold.net>
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

use core::arch::global_asm;
use core::mem::offset_of;

global_asm!(
    ".section .init.reset, \"ax\"",
    ".global _reset",
    "_reset:",
        // Load magic
        "li     a0, {MEM_OFFSET}",
        "ld     t0, {MAGIC_OFFSET}(a0)",
        // Check if magic is equal to BROM_HDR_MAGIC
        "li     t1, {BROM_HDR_MAGIC}",
        "beq    t0, t1, 1f",
        // Context not initialized, jump to code in ROM
        "j      _start",
        // Load entry address
    "1: ld      ra, {ENTRY_ADDR_OFFSET}(a0)",
        // TODO: Lock UDS access to be sure (should be locked already)
        "ret",
    MEM_OFFSET = const rot::LayerCtx::<()>::MEM_OFFSET,
    BROM_HDR_MAGIC = const rot::LayerCtx::<()>::BROM_HDR_MAGIC,
    MAGIC_OFFSET = const offset_of!(rot::LayerCtx::<()>, brom_hdr_magic),
    ENTRY_ADDR_OFFSET = const offset_of!(rot::LayerCtx::<()>, entry_addr),
);
