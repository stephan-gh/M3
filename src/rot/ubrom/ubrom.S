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

#include <base/Config.h>

#define TCU_MMIO_ADDR       0xF0000000
#define _TCU_EXT_REGS       0
#define TCU_TILE_DESC       (_TCU_EXT_REGS + (1 * 8))
#define _TCU_UNPRIV_REGS    (_TCU_EXT_REGS + (5 * 8))
#define TCU_COMMAND         (_TCU_UNPRIV_REGS + (0 * 8))
#define TCU_DATA_ADDR       (_TCU_UNPRIV_REGS + (1 * 8))
#define TCU_DATA_SIZE       (_TCU_UNPRIV_REGS + (2 * 8))
#define TCU_DATA_ARG1       (_TCU_UNPRIV_REGS + (3 * 8))

#define TCU_CMD_IDLE        0
#define TCU_CMD_READ        3

#define KECACC_MMIO_ADDR        0xF4200000
#define KECACC_CMD_INIT         1
#define KECACC_CMD_ABSORB       4
#define KECACC_CMD_ABSORB_LAST  5
#define KECACC_CMD_SQUEEZE      6

#define KMAC_HASH_TYPE      8
#define KMAC_RATE           (1088 / 8)

#define KMAC_CDI_OFFSET     24 // BromLayerCtx::kmac_cdi offset

#define CDI_SIZE            (512 / 8)

.macro print text, len
#if defined(__gem5__) && !defined(bench)
    la      a0, \text
    li      a1, \len
    li      a2, 0
    la      a3, stdout
    .long   0x9E00007B
#endif
.endm

/*
 * Register usage:
 *   a0 = MEM_OFFSET (beginning of memory and layer context pointer)
 *   a1 = address of loaded RoT configuration (end of memory - PAGE_SIZE)
 *   s0 = LayerCtx::BROM_HDR_MAGIC
 *   s1 = TCU_MMIO_ADDR
 *   s2 = TCU_CMD_READ, later KECACC_MMIO_ADDR
 *   s3 = BromCfg::size (size of next layer)
 *   tN = temporary registers
 *   ra = entry address of next layer
 */
.global _reset
_reset:
    print   hello, 25

    la      gp, rodata_begin

    // Load magic
    li      a0, MEM_OFFSET
    ld      t0, 0(a0) // LayerCtx::brom_hdr_magic
    // Check if magic is equal to BROM_HDR_MAGIC
    ld      s0, 0(gp) // LayerCtx::BROM_HDR_MAGIC
    bne     t0, s0, 1f
    // Load entry address
    ld      ra, 8(a0) // LayerCtx::entry_addr
    // TODO: Lock UDS access to be sure (should be locked already)
    ret

1:  // Context not initialized

    // Find end of memory for RoT configuration from flash
    li      s1, TCU_MMIO_ADDR
    ld      t0, TCU_TILE_DESC(s1)
    srli    t0, t0, 28
    slli    t0, t0, 12
    add     a1, a0, t0
    li      t0, (1 << PAGE_BITS)
    sub     a1, a1, t0

    // Load RoT configuration from flash
    sd      a1, TCU_DATA_ADDR(s1)
    sd      t0, TCU_DATA_SIZE(s1)
    sd      zero, TCU_DATA_ARG1(s1)
    li      s2, TCU_CMD_READ
    sd      s2, TCU_COMMAND(s1)

    // Wait for RoT configuration to be loaded
1:  ld      t0, TCU_COMMAND(s1)
    bnez    t0, 1b
    fence   io,r // Make following read operations only after TCU command is complete

    // Check magic
    ld      t0, 0(a1)
    ld      t1, 8(gp) // BromCfg::MAGIC
    bne     t0, t1, panic

    li      ra, (MEM_OFFSET + 0x3000)
    lw      t0, 8(a1) // BromCfg::flash_offset
    lw      s3, 12(a1) // BromCfg::size

    // Load next RoT layer from flash
    sd      ra, TCU_DATA_ADDR(s1)
    sd      s3, TCU_DATA_SIZE(s1)
    sd      t0, TCU_DATA_ARG1(s1)
    sd      s2, TCU_COMMAND(s1)

    // Wait for RoT layer to be loaded
1:  ld      t0, TCU_COMMAND(s1)
    bnez    t0, 1b
    fence   io,w // Make sure RoT layer completed loading

    // Append KMAC output length
    lw      t0, 56(gp)
    add     t1, ra, s3
    sw      t0, 0(t1)
    addi    s3, s3, 3 // Also absorb KMAC output length

    // Initialize hash accelerator
    li      s2, KECACC_MMIO_ADDR
    li      t0, (KECACC_CMD_INIT | (KMAC_HASH_TYPE << 4))
    sd      t0, 0(s2)

    // Copy cSHAKE and KMAC header
    ld      t1, 24(gp)
    ld      t2, 32(gp)
    ld      t3, 40(gp)
    sd      t1, (KMAC_CDI_OFFSET + 0 + 0)(a0)
    sd      t2, (KMAC_CDI_OFFSET + 0 + 8)(a0)
    sd      t3, (KMAC_CDI_OFFSET + KMAC_RATE + 0)(a0)

    // Wait for accelerator to finish
1:  ld      t1, 0(s2)
    bnez    t1, 1b

    // Prepare KMAC key with dummy UDS (all zeroes)
    // To simulate MMIO latency the UDS is read from the KecAcc command register
    ld      t0, 0(s2)
    sd      t0, (KMAC_CDI_OFFSET + KMAC_RATE + 8)(a0)
    ld      t0, 0(s2)
    sd      t0, (KMAC_CDI_OFFSET + KMAC_RATE + 16)(a0)
    ld      t0, 0(s2)
    sd      t0, (KMAC_CDI_OFFSET + KMAC_RATE + 24)(a0)
    ld      t0, 0(s2)
    sd      t0, (KMAC_CDI_OFFSET + KMAC_RATE + 32)(a0)
    li      t0, 0
    // SECURITY: UDS now stored in (KMAC_CDI_OFFSET + KMAC_RATE + (8 to 40))(a0)
    // TODO: Lock UDS access (there is no actual UDS at the moment)

    // Absorb
    fence   w,o // Finish writing before starting accelerator
    li      t0, (KECACC_CMD_ABSORB | (MEM_OFFSET + KMAC_CDI_OFFSET) << 4 | (KMAC_RATE * 2) << 34)
    sd      t0, 0(s2)
1:  ld      t1, 0(s2)
    bnez    t1, 1b
    fence   io,w // Finish accelerator before writing to input

    // Explicitly clear UDS from memory
    sd      zero, (KMAC_CDI_OFFSET + KMAC_RATE + 8)(a0)
    sd      zero, (KMAC_CDI_OFFSET + KMAC_RATE + 16)(a0)
    sd      zero, (KMAC_CDI_OFFSET + KMAC_RATE + 24)(a0)
    sd      zero, (KMAC_CDI_OFFSET + KMAC_RATE + 32)(a0)
    fence
    // SECURITY: UDS now cleared from memory

    // Absorb loaded boot layer
    slli    t0, ra, 4
    slli    t1, s3, 34
    or      t0, t0, t1
    ori     t0, t0, KECACC_CMD_ABSORB_LAST
    sd      t0, 0(s2)
1:  ld      t1, 0(s2)
    bnez    t1, 1b

    // Prepare new KMAC header for CDI
    ld      t0, 48(gp)
    sd      t0, KMAC_CDI_OFFSET(a0)

    // Squeeze CDI
    li      t0, (KECACC_CMD_SQUEEZE | (MEM_OFFSET + 24 + 8) << 4 | CDI_SIZE << 34)
    sd      t0, 0(s2)
1:  ld      t1, 0(s2)
    bnez    t1, 1b

    // Reset accelerator
    li      t0, KECACC_CMD_INIT
    sd      t0, 0(s2)

    // Initialize rest of layer context
    sd      s0, 0(a0) // LayerCtx::brom_hdr_magic
    sd      ra, 8(a0) // LayerCtx::entry_addr
    ld      t0, 16(gp) // BromCtx::MAGIC
    sd      t0, 16(a0) // LayerCtx::magic

    // Wait for accelerator to finish
1:  ld      t1, 0(s2)
    bnez    t1, 1b
    fence
    ret

panic:
    j       panic

.section .rodata
    .align 3
rodata_begin:
    .8byte  0x42726f6d48647201 // LayerCtx::BROM_HDR_MAGIC
    .8byte  0x42726f6d43666701 // BromCfg::MAGIC
    .8byte  0x42726f6d43747801 // BromCtx::MAGIC
// 24: cshake header
    .byte   0x01, 0x88 /* bytepad() */
    .byte   0x01, 0x20 /* left_encode(len("KMAC") * 8) */
    .ascii  "KMAC"
    .byte   0x01, 0x20 /* left_encode(len("DICE") * 8) */
    .ascii  "DICE"
    .align  3
// 40: KMAC key
    .byte   0x01, 0x88 /* bytepad() */
    .byte   0x02, 0x01, 0x18 /* left_encode(256 + 3*8) */
    .ascii  "UDS" /* Padding */
    .align  3
// 48: KMAC key for CDI
    .byte   0x01, 0x88 /* bytepad() */
    .byte   0x02, 0x02, 0x18 /* left_encode(512 + 3*8) */
    .ascii  "CDI" /* Padding */
    .align  2
// 56: KMAC output length
    .byte   0x02, 0x00, 0x02
    .align  2

#if defined(__gem5__) && !defined(bench)
hello:
    .ascii "Hello World from μbrom!\n" // Length 25
stdout:
    .asciz "stdout"
#endif
