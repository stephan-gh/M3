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

use core::arch::asm;
use core::mem::size_of;

use base::io::LogFlags;
use base::{cfg, log};

use crate::{CtxData, LayerCtx};

// If the context is not at the beginning of the memory, the assembly needs to be changed
// so that the beginning of SRAM is still cleared if needed. Right now the context is
// copied to the beginning of SRAM and the rest of the stack is cleared.
const _: () = assert!(
    LayerCtx::<()>::MEM_OFFSET == cfg::MEM_OFFSET,
    "Assembly needs changes if context is moved"
);

pub(crate) unsafe fn switch<Data: CtxData>(ctx: LayerCtx<Data>) -> ! {
    log!(
        LogFlags::RoTBoot,
        "Jumping to next layer @ {:#x}",
        ctx.entry_addr
    );
    asm!(
        // Copy the context to the beginning of memory
        "1: ld      x4, 0({ctx_pos})",
        "   addi    {ctx_pos}, {ctx_pos}, 8",
        "   sd      x4, 0({mem_pos})",
        "   addi    {mem_pos}, {mem_pos}, 8",
        "   bne     {mem_pos}, {copy_end}, 1b",

        // Clear the rest of the stack and the BSS
        "   la      x4, _eclear",
        "2: sd      zero, 0({mem_pos})",
        "   addi    {mem_pos}, {mem_pos}, 8",
        "   bne     {mem_pos}, x4, 2b",

        // Clear registers
        //" li      x1, 0", // Contains entry address
        "   li      x2, 0",
        "   li      x3, 0",
        "   li      x4, 0",
        "   li      x5, 0",
        "   li      x6, 0",
        "   li      x7, 0",
        "   li      x8, 0",
        "   li      x9, 0",
        "   li      x10, 0",
        "   li      x11, 0",
        "   li      x12, 0",
        "   li      x13, 0",
        "   li      x14, 0",
        "   li      x15, 0",
        "   li      x16, 0",
        "   li      x17, 0",
        "   li      x18, 0",
        "   li      x19, 0",
        "   li      x20, 0",
        "   li      x21, 0",
        "   li      x22, 0",
        "   li      x23, 0",
        "   li      x24, 0",
        "   li      x25, 0",
        "   li      x26, 0",
        "   li      x27, 0",
        "   li      x28, 0",
        "   li      x29, 0",
        "   li      x30, 0",
        "   li      x31, 0",
        "   fence",

        // Jump to the new entry point
        "   ret",

        in("x1") ctx.entry_addr, // ra
        ctx_pos = in(reg) &ctx,
        mem_pos = in(reg) cfg::MEM_OFFSET,
        copy_end = in(reg) cfg::MEM_OFFSET + size_of::<LayerCtx<Data>>(),
        options(noreturn)
    )
}

pub(crate) unsafe fn sleep<Data: CtxData>(ctx: &LayerCtx<Data>) -> ! {
    log!(
        LogFlags::RoTBoot,
        "Sleeping until external reset to next layer @ {:#x}",
        ctx.entry_addr
    );
    loop {
        asm!(
            // Dummy usage to make sure context is not discarded
            "/* {ctx_pos} */",
            "wfi",
            ctx_pos = in(reg) ctx
        );
    }
}
