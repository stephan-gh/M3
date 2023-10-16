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

use base::col::{BTreeMap, BTreeMapEntry, Vec};
use base::io::log::LogColor;
use base::io::{log, LogFlags};
use base::kif::boot::{Info, Mem, Mod};
use base::kif::{Perm, TileAttr, TileType};
use base::mem::{GlobAddr, GlobOff};
use base::tcu::TCU;
use base::util::math::round_up;
use base::{cfg, env, log, mem, tcu, util};
use rot::cert::{HashBuf, M3RawCertificate};
use rot::ed25519::{Signer, SigningKey};
use rot::{Hex, Secret};

fn config_local_ep<CFG>(ep: tcu::EpId, cfg: CFG)
where
    CFG: FnOnce(&mut [tcu::Reg]),
{
    let mut regs = [0; tcu::EP_REGS];
    cfg(&mut regs);
    TCU::set_ep_regs(ep, &regs);
}

fn config_remote_ep<CFG>(rtcu_ep: tcu::EpId, ep: tcu::EpId, cfg: CFG)
where
    CFG: FnOnce(&mut [tcu::Reg]),
{
    let mut regs = [0; tcu::EP_REGS];
    cfg(&mut regs);
    let off = (TCU::ep_regs_addr(ep) - tcu::MMIO_ADDR).as_goff();
    TCU::write_slice(rtcu_ep, &regs[..], off).expect("Failed to configure remote TCU endpoint");
}

fn config_local_ep_remote_tcu(noc_id: u16, perm: Perm) {
    config_local_ep(crate::TILE_EP, |regs| {
        TCU::config_mem_raw(
            regs,
            rot::TCU_ACT_ID,
            noc_id,
            tcu::MMIO_ADDR.as_goff(),
            tcu::MMIO_SIZE,
            perm,
        );
    });
}

/// Helper macro to find the best position in an iterator that satisfies a condition.
/// The base condition must always be satisfied, the preferred conditions are tried
/// one by one until one is satisfied or none are left.
///
/// The current implementation is not very efficient, it iterates several times
/// checking the base condition over and over again.
macro_rules! find_best_position {
    ($iter:expr, |$name:ident| $base_cond:expr) => {
        $iter.position(|$name| $base_cond)
    };
    ($iter:expr, |$name:ident| $base_cond:expr,
     try => $prefer_cond:expr $(, try => $cond_tail:expr)* $(,)?) => {
        $iter.position(|$name| $base_cond && $prefer_cond)
            .or_else(|| find_best_position!($iter, |$name| $base_cond $(, try => $cond_tail)*))
    };
}

pub fn main() -> ! {
    log::init(env::boot().tile_id(), "rosa", LogColor::BrightMagenta);
    log!(LogFlags::RoTBoot, "Hello World");

    let ctx = unsafe { rot::BlauLayerCtx::take() };
    let cfg = unsafe { rot::RosaLayerCfg::get() };

    log!(LogFlags::RoTBoot, "Scanning tiles");
    let tiles = env::boot().raw_tile_ids[0..env::boot().raw_tile_count as usize]
        .iter()
        .map(|id| {
            config_local_ep_remote_tcu(*id as u16, Perm::R);
            TCU::read_obj(
                crate::TILE_EP,
                (TCU::ext_reg_addr(tcu::ExtReg::TileDesc) - tcu::MMIO_ADDR).as_goff(),
            )
            .expect("Failed to read tile desc")
        })
        .collect();
    log!(LogFlags::RoTDbg, "Tiles: {:#?}", tiles);

    let mut m3 = rot::cert::M3Payload {
        tiles,
        kernel: rot::cert::M3KernelConfig {
            mem_size: cfg.data.kernel_mem_size,
            cmdline: util::cstr_slice_to_str(&cfg.data.kernel_cmdline),
        },
        mods: BTreeMap::new(),
        pub_key: Hex::new_zeroed(),
    };

    // We just use the first mem tile for now and assume it has sufficient space
    let mem_tile_pos = m3
        .tiles
        .iter()
        .position(|t| t.tile_type() == TileType::Mem)
        .expect("Failed to find mem tile");
    let mem_size = m3.tiles[mem_tile_pos].mem_size();
    let mem_tile_raw = env::boot().raw_tile_ids[mem_tile_pos] as u16;
    let mem_tile = TCU::nocid_to_tileid(mem_tile_raw);
    log!(LogFlags::RoTBoot, "Picked memory tile: {}", mem_tile);

    // Configure memory endpoint that spans the entire memory tile
    config_local_ep(crate::MEM_EP, |regs| {
        TCU::config_mem_raw(regs, rot::TCU_ACT_ID, mem_tile_raw, 0, mem_size, Perm::W)
    });

    // Load modules
    let mod_count = cfg.data.mod_count();
    let mut mods = Vec::with_capacity(mod_count + 1);
    let mut mem_offset = 0;
    {
        // SAFETY: COPY_BUF is only used in the (single-threaded) main boot path
        let copy_buf = unsafe { crate::COPY_BUF.get_mut() };
        for m in &cfg.data.mods[0..mod_count] {
            let mname = m.name();
            let msize = m.size as usize;

            log!(
                LogFlags::RoTBoot,
                "Copying and hashing mod {} ({} KiB): {} -> {}",
                mname,
                msize / 1024,
                m.addr(),
                GlobAddr::new_with(mem_tile, mem_offset)
            );

            // Make sure we don't read anything from inside the RoT tile
            assert_ne!(m.addr().tile(), env::boot().tile_id());

            let mut hash: Hex<HashBuf> = Hex::new_zeroed();
            config_local_ep(crate::COPY_EP, |regs| {
                TCU::config_mem(
                    regs,
                    rot::TCU_ACT_ID,
                    m.addr().tile(),
                    m.addr().offset(),
                    msize,
                    Perm::R,
                )
            });
            rot::copy_and_hash(
                rot::cert::HASH_TYPE,
                crate::COPY_EP,
                crate::MEM_EP,
                mem_offset,
                msize,
                &mut copy_buf[..],
                &mut hash[..],
            );
            log!(LogFlags::RoTBoot, "Hash: {}", hash);

            match m3.mods.entry(mname) {
                BTreeMapEntry::Vacant(e) => e.insert(hash),
                BTreeMapEntry::Occupied(entry) => {
                    log!(
                        LogFlags::Error,
                        "Duplicate module {} with previous hash: {:?}. Skipping.",
                        mname,
                        entry.get()
                    );
                    continue;
                },
            };

            let new_addr = GlobAddr::new_with(mem_tile, mem_offset);
            mods.push(Mod::new(new_addr, m.size, mname));
            mem_offset = round_up(mem_offset + msize as GlobOff, cfg::PAGE_SIZE as GlobOff);
        }
    }

    log!(LogFlags::RoTDbg, "Loaded modules: {:#?}", mods);
    log!(LogFlags::RoTDbg, "Module hashes: {:#?}", m3.mods);

    // Prepare next context
    let mut next_ctx = rot::RosaCtx {
        kmac_cdi: Secret::new_zeroed(),
        derived_private_key: Secret::new_zeroed(),
    };
    {
        let cdi_json = rot::json::to_string(&m3).expect("Failed to serialize config for CDI");
        let cdi_bytes = cdi_json.as_bytes();
        log!(
            LogFlags::RoTDbg,
            "CDI JSON ({} bytes): {}",
            cdi_json.as_bytes().len(),
            cdi_json,
        );
        rot::derive_cdi(&ctx.data.kmac_cdi, cdi_bytes, &mut next_ctx.kmac_cdi);
    }
    m3.pub_key = Hex({
        rot::derive_key(
            &next_ctx.kmac_cdi,
            "ED25519",
            &[],
            &mut next_ctx.derived_private_key.secret[..],
        );
        let next_sig_key = SigningKey::from_bytes(&next_ctx.derived_private_key.secret);
        log!(LogFlags::RoTDbg, "Derived next layer {:?}", next_sig_key);
        next_sig_key.verifying_key().to_bytes()
    });

    {
        let sign_raw = rot::json::value::to_raw_value(&m3).unwrap();
        log!(
            LogFlags::RoTDbg,
            "JSON to be signed ({} bytes): {}",
            sign_raw.get().as_bytes().len(),
            sign_raw.get(),
        );

        let sig_key = SigningKey::from_bytes(&ctx.data.derived_private_key.secret);
        let signature = Hex(sig_key.sign(sign_raw.get().as_bytes()).to_bytes());
        log!(LogFlags::RoTDbg, "Signed: {}", signature);

        let cert = M3RawCertificate {
            payload: sign_raw,
            signature,
            pub_key: Hex(sig_key.verifying_key().to_bytes()),
            parent: rot::cert::Certificate {
                payload: ctx.data.signed_payload,
                signature: ctx.data.signature,
                pub_key: ctx.data.signer_public_key,
                parent: (),
            },
        };
        let cert_json = rot::json::to_string(&cert).expect("Failed to serialize certificate");
        let cert_json_size = cert_json.as_bytes().len();
        log!(
            LogFlags::RoTDbg,
            "rot-certificate.json ({} bytes): {}",
            cert_json_size,
            cert_json,
        );

        TCU::write_slice(1, cert_json.as_bytes(), mem_offset)
            .expect("Failed to write rot-certificate.json to DRAM");

        mods.push(Mod::new(
            GlobAddr::new_with(mem_tile, mem_offset),
            cert_json_size as u64,
            "rot-certificate.json",
        ));
        mem_offset += cert_json_size as GlobOff;
        mem_offset = round_up(mem_offset, cfg::PAGE_SIZE as GlobOff);
    }

    const MEM_COUNT: usize = 1;
    let total_env_size = mem::size_of::<Info>()
        + mem::size_of_val(&mods[..])
        + mem::size_of_val(&m3.tiles[..])
        + mem::size_of::<Mem>() * MEM_COUNT;

    let kenv_offset = mem_offset;
    mem_offset += total_env_size as GlobOff;
    let kenv_end = mem_offset;
    mem_offset = round_up(mem_offset, cfg::PAGE_SIZE as GlobOff);
    let kernel_offset = mem_offset;
    mem_offset += m3.kernel.mem_size as GlobOff;

    {
        let mems: [Mem; MEM_COUNT] = [Mem::new(
            GlobAddr::new_with(mem_tile, mem_offset),
            mem_size as GlobOff - mem_offset,
            false,
        )];
        let info = Info {
            mod_count: mods.len() as u64,
            tile_count: m3.tiles.len() as u64,
            mem_count: mems.len() as u64,
            serv_count: 0,
        };
        log!(LogFlags::RoTDbg, "Boot {:?}", info);

        let mut off = kenv_offset;
        TCU::write_obj(crate::MEM_EP, &info, off).expect("Failed to write boot info");
        off += mem::size_of::<Info>() as GlobOff;
        TCU::write_slice(crate::MEM_EP, &mods[..], off).expect("Failed to write mods");
        off += mem::size_of_val(&mods[..]) as GlobOff;
        TCU::write_slice(crate::MEM_EP, &m3.tiles[..], off).expect("Failed to write tiles");
        off += mem::size_of_val(&m3.tiles[..]) as GlobOff;
        TCU::write_slice(crate::MEM_EP, &mems[..], off).expect("Failed to write mems");
        off += mem::size_of_val(&mems[..]) as GlobOff;
        assert_eq!(off, kenv_end);
    };

    {
        // Find kernel module and configure endpoint for loading
        let kmod = mods
            .iter()
            .find(|&m| m.name() == "kernel")
            .expect("Failed to find kernel mod");
        log!(LogFlags::RoTBoot, "Found kernel: {:?}", kmod);

        config_local_ep(crate::COPY_EP, |regs| {
            TCU::config_mem(
                regs,
                rot::TCU_ACT_ID,
                kmod.addr().tile(),
                kmod.addr().offset(),
                kmod.size as usize,
                Perm::R,
            )
        });
    }

    let ktile_idx = {
        find_best_position!(
            m3.tiles.iter(),
            |desc| desc.is_programmable() && !desc.attr().contains(TileAttr::ROT),
            try => desc.has_virtmem() && desc.attr().contains(TileAttr::EFFI),
            try => desc.has_virtmem(),
            try => desc.attr().contains(TileAttr::EFFI),
        )
        .expect("No suitable tile found for kernel")
    };
    let ktile_raw = env::boot().raw_tile_ids[ktile_idx] as u16;
    let ktile = TCU::nocid_to_tileid(ktile_raw);
    assert_ne!(ktile, env::boot().tile_id());
    log!(
        LogFlags::RoTBoot,
        "Picked kernel tile {} with desc: {:?}, configuring endpoints",
        ktile,
        m3.tiles[ktile_idx]
    );

    // Configure endpoint to kernel TCU
    config_local_ep_remote_tcu(ktile_raw, Perm::RW);

    // Configure kernel memory endpoint
    config_remote_ep(crate::TILE_EP, 0, |regs| {
        TCU::config_mem_raw(
            regs,
            rot::TCU_ACT_ID,
            mem_tile_raw,
            kernel_offset,
            m3.kernel.mem_size as usize,
            Perm::RWX,
        )
    });
    // Configure endpoint used to load kernel ELF
    config_local_ep(crate::MEM_EP, |regs| {
        TCU::config_mem_raw(
            regs,
            rot::TCU_ACT_ID,
            ktile_raw,
            cfg::MEM_OFFSET as GlobOff,
            m3.kernel.mem_size as usize,
            Perm::W,
        )
    });
    // Configure endpoint used to load kernel environment
    config_local_ep(crate::ENV_EP, |regs| {
        TCU::config_mem_raw(
            regs,
            rot::TCU_ACT_ID,
            ktile_raw,
            cfg::MEM_ENV_START.as_goff(),
            cfg::ENV_SIZE,
            Perm::W,
        )
    });

    // Continue loading in second stage after clearing secrets
    let next_ctx = rot::LayerCtx::new(rot::ROSA_ADDR, crate::RosaPrivateCtx {
        next: next_ctx,
        kernel_tile_id: ktile.raw() as u64,
        kernel_tile_desc: m3.tiles[ktile_idx].value(),
        kenv_addr: GlobAddr::new_with(mem_tile, kenv_offset),
    });
    unsafe { next_ctx.switch() }
}
