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

MEMORY
{
    STACK : ORIGIN = 0x10000000, LENGTH = 16K
    BSS : ORIGIN = 0x10004000, LENGTH = 80K
    RAM : ORIGIN = 0x10018000, LENGTH = 156K
}

REGION_ALIAS("REGION_TEXT", RAM);
REGION_ALIAS("REGION_RODATA", RAM);
REGION_ALIAS("REGION_DATA", BSS); /* Copied from RAM at startup */
REGION_ALIAS("REGION_BSS", BSS);
REGION_ALIAS("REGION_HEAP", BSS);
REGION_ALIAS("REGION_STACK", STACK);

/*
 * Secrets are cleared between stage1 and stage2 where rosa is re-entered.
 * The initialization routine in riscv-rt copies .data and clears .bss,
 * so there is no need to clear it again before the switch.
 */
PROVIDE(_eclear = _sstack);
