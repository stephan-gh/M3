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

use core::cmp::min;

use base::elf::{ElfHeader, PHType, ProgramHeader};
use base::env::BootEnv;
use base::io::log::LogColor;
use base::io::{log, LogFlags};
use base::mem::{GlobOff, VirtAddr};
use base::tcu::TCU;
use base::util::math::round_up;
use base::{cfg, env, log, mem, tcu, util};
use rot::{CtxData, RosaCtx};

fn write_args<S, I>(args: I, env_off: &mut GlobOff) -> (VirtAddr, usize)
where
    S: AsRef<str>,
    I: IntoIterator<Item = S>,
{
    let (arg_buf, arg_ptrs, _) = env::collect_args(args, cfg::MEM_ENV_START + *env_off);
    TCU::write_slice(crate::ENV_EP, &arg_buf[..], *env_off)
        .expect("Failed to write arguments to kernel tile");
    *env_off = round_up(
        *env_off + mem::size_of_val(&arg_buf[..]) as GlobOff,
        mem::size_of::<VirtAddr>() as GlobOff,
    );
    TCU::write_slice(crate::ENV_EP, &arg_ptrs[..], *env_off)
        .expect("Failed to write argument pointers to kernel tile");
    let argp = cfg::MEM_ENV_START + *env_off;
    *env_off += mem::size_of_val(&arg_ptrs[..]) as GlobOff;
    (argp, arg_ptrs.len() - 1)
}

pub fn main() -> ! {
    log::init(env::boot().tile_id(), "rosa2", LogColor::Magenta);
    log!(LogFlags::RoTBoot, "Hello World");

    let ctx = unsafe { crate::RosaPrivateLayerCtx::get() };
    log!(LogFlags::RoTDbg, "{:#x?}", ctx);
    let cfg = unsafe { rot::RosaLayerCfg::get() };

    {
        log!(LogFlags::RoTBoot, "Loading kernel");
        let hdr: ElfHeader =
            TCU::read_obj(crate::COPY_EP, 0).expect("Failed to read kernel ELF header");
        log!(LogFlags::RoTDbg, "{:x?}", hdr);
        assert_eq!(&hdr.ident[..4], b"\x7FELF", "Invalid ELF magic");
        assert!(
            hdr.ph_entry_size as usize >= mem::size_of::<ProgramHeader>(),
            "Unexpected size of program header entries"
        );

        // SAFETY: COPY_BUF is only used in the (single-threaded) main boot path
        let copy_buf = unsafe { crate::COPY_BUF.get_mut() };
        let mut ph_off = hdr.ph_off;
        for _ in 0..hdr.ph_num {
            let phdr: ProgramHeader = TCU::read_obj(crate::COPY_EP, ph_off as GlobOff)
                .expect("Failed to read ELF program header");
            ph_off += hdr.ph_entry_size as usize;
            log!(LogFlags::RoTDbg, "{:x?}", phdr);

            if phdr.ty != PHType::Load as u32 || phdr.mem_size == 0 {
                continue;
            }
            assert!(phdr.mem_size >= phdr.file_size);

            let mut size = phdr.mem_size as usize;
            let mut off = (phdr.phys_addr - cfg::MEM_OFFSET) as GlobOff;
            if phdr.file_size > 0 {
                let mut copy = min(size, phdr.file_size as usize);
                size -= copy;

                let mut elf_off = phdr.offset as GlobOff;
                while copy > 0 {
                    let len = min(copy, copy_buf.len());
                    TCU::read(crate::COPY_EP, copy_buf.as_mut_ptr(), len, elf_off)
                        .expect("Failed to read ELF segment data from memory");
                    TCU::write(crate::MEM_EP, copy_buf.as_ptr(), len, off)
                        .expect("Failed to write ELF segment data to kernel tile");
                    elf_off += len as GlobOff;
                    off += len as GlobOff;
                    copy -= len;
                }
            }

            // BSS
            crate::clear_mem(off, size).expect("Failed to write BSS to kernel tile");
        }
    }

    {
        // Copy kernel arguments and environment variables
        let mut env_off = mem::size_of::<BootEnv>() as GlobOff;
        let kernel_cmdline = util::cstr_slice_to_str(&cfg.data.kernel_cmdline);
        let (argv, argc) = write_args(kernel_cmdline.split(' '), &mut env_off);
        let (envp, _) = write_args(env::Vars::default(), &mut env_off);

        let env = BootEnv {
            platform: env::boot().platform,
            tile_id: ctx.data.kernel_tile_id,
            tile_desc: ctx.data.kernel_tile_desc,
            argc: argc as u64,
            argv: argv.as_raw(),
            envp: envp.as_raw(),
            kenv: ctx.data.kenv_addr.raw(),
            raw_tile_count: env::boot().raw_tile_count,
            raw_tile_ids: env::boot().raw_tile_ids,
        };
        log!(LogFlags::RoTDbg, "{:x?}", env);
        TCU::write_obj(crate::ENV_EP, &env, 0).expect("Failed to write BootEnv to kernel tile");
    }

    // Fixup context
    ctx.entry_addr = rot::ROSA_NEXT_ADDR as u64;
    ctx.magic = RosaCtx::MAGIC;

    {
        log!(LogFlags::RoTBoot, "Resetting kernel tile");
        let ext_cmd_addr = (TCU::ext_reg_addr(tcu::ExtReg::ExtCmd) - tcu::MMIO_ADDR).as_goff();
        let reset_val = tcu::ExtCmdOpCode::Reset as tcu::Reg | (1 << 9) as tcu::Reg;
        TCU::write_obj(crate::TILE_EP, &reset_val, ext_cmd_addr)
            .expect("Failed to write kernel reset");

        let res = loop {
            let res: tcu::Reg = TCU::read_obj(crate::TILE_EP, ext_cmd_addr)
                .expect("Failed to read kernel reset status");
            if (res & 0xF) == tcu::ExtCmdOpCode::Idle as tcu::Reg {
                break res;
            }
        };
        log!(LogFlags::RoTDbg, "Reset command complete: {}", res);
    }

    unsafe { ctx.sleep() }
}
