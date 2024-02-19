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

use core::any::type_name;
use core::fmt::Debug;

use base::io::LogFlags;
use base::kif::boot::Mod;
use base::kif::{TileAttr, TileDesc};
use base::mem::{size_of, GlobOff};
use base::{cfg, env, log};

use crate::{ed25519, encode_magic, Hex, Magic, OpaqueKMacKey, Secret};

#[repr(C)]
#[derive(Debug)]
pub struct SimpleBinaryCfg {
    pub flash_offset: u32,
    pub size: u32,
}

#[repr(C)]
#[derive(Debug)]
pub struct CachedCertificateCfg {
    pub pub_key: Hex<[u8; ed25519::PUBLIC_KEY_LENGTH]>,
    pub signature: Hex<[u8; ed25519::SIGNATURE_LENGTH]>,
}

#[repr(C)]
#[derive(Debug)]
pub struct Mac<T> {
    pub data: T,
    pub mac: Hex<[u8; 64]>,
}

#[repr(C)]
#[derive(Debug)]
pub struct LayerCfg<Prev: CheckMagic, Data: CfgData> {
    pub prev: Prev,
    pub magic: Magic,
    pub data: Data,
}

pub trait CfgData: Debug {
    const MAGIC: Magic;
}

#[repr(C)]
#[derive(Debug)]
pub struct BromCfg {
    pub next_layer: SimpleBinaryCfg,
}

impl CfgData for BromCfg {
    const MAGIC: Magic = encode_magic(b"BromCfg", 1);
}

#[repr(C)]
#[derive(Debug)]
pub struct BlauCfg {
    pub next_layer: SimpleBinaryCfg,
    pub cache: Mac<Hex<[u8; ed25519::PUBLIC_KEY_LENGTH]>>,
    pub next_cache: Mac<CachedCertificateCfg>,
}

impl CfgData for BlauCfg {
    const MAGIC: Magic = encode_magic(b"BlauCfg", 3);
}

#[repr(C)]
#[derive(Debug)]
pub struct RosaCfg {
    pub next_cache: Mac<CachedCertificateCfg>,
    pub kernel_mem_size: GlobOff,
    pub kernel_ep_pages: u8,
    // Can reduce this a bit more to free up space, or reduce number of boot modules
    pub kernel_cmdline: [u8; 31],
    pub mods: [Mod; Self::MAX_MODS],
}

impl RosaCfg {
    pub const MAX_MODS: usize = 45;

    pub fn mod_count(&self) -> usize {
        self.mods.iter().take_while(|&m| m.size != 0).count()
    }
}

impl CfgData for RosaCfg {
    const MAGIC: Magic = encode_magic(b"RosaCfg", 3);
}

pub type BromLayerCfg = LayerCfg<(), BromCfg>;
pub type BlauLayerCfg = LayerCfg<BromLayerCfg, BlauCfg>;
pub type RosaLayerCfg = LayerCfg<BlauLayerCfg, RosaCfg>;

const RESERVED_SIZE: usize = cfg::PAGE_SIZE;
const _: () = assert!(
    size_of::<RosaLayerCfg>() <= RESERVED_SIZE,
    "Layer configuration too large"
);

pub trait CheckMagic: Debug {
    fn check_magic(&self);
}

impl CheckMagic for () {
    fn check_magic(&self) {
    }
}

impl<Prev: CheckMagic, Data: CfgData> CheckMagic for LayerCfg<Prev, Data> {
    fn check_magic(&self) {
        assert_eq!(
            self.magic,
            Data::MAGIC,
            "{} magic is invalid",
            type_name::<Data>()
        );
        self.prev.check_magic();
    }
}

impl<Prev: CheckMagic, Data: CfgData> LayerCfg<Prev, Data> {
    /// Get a reference to the layer configuration loaded by the Boot ROM.
    ///
    /// # Safety
    /// The caller must ensure that the configuration is accessible at the
    /// reserved address. This is generally only the case for the RoT tile
    /// where the Boot ROM has loaded the configuration.
    ///
    /// **IMPORTANT: The configuration may be INVALID or MANIPULATED. All
    /// fields can have any arbitrary value. Careful checks are required to
    /// ensure a manipulated configuration does not lead to secret disclosure!**
    pub unsafe fn get() -> &'static mut Self {
        let cfg = unsafe { &mut *(reserved_addr() as *mut Self) };
        cfg.check_magic();
        log!(LogFlags::RoTDbg, "{:#x?}", cfg);
        cfg
    }
}

fn reserved_addr() -> usize {
    let desc = TileDesc::new_from(env::boot().tile_desc);
    assert!(desc.attr().contains(TileAttr::IMEM));
    cfg::MEM_OFFSET + desc.mem_size() - RESERVED_SIZE
}

/// Get a mutable reference to the reserved memory area.
///
/// # Safety
/// The caller must ensure that the memory region is valid and is not used by
/// the current program.
pub unsafe fn cfg_reservation() -> &'static mut [u8; RESERVED_SIZE] {
    unsafe { &mut *(reserved_addr() as *mut [u8; RESERVED_SIZE]) }
}

impl<T> Mac<T> {
    fn mac(&self, key: &Secret<OpaqueKMacKey>) -> Hex<[u8; 64]> {
        let mut mac: Hex<[u8; 64]> = Hex::new_zeroed();
        crate::cdi::derive_key(key, "CacheMAC", self.data_as_bytes(), &mut mac[..]);
        mac
    }

    pub fn check(&self, key: &Secret<OpaqueKMacKey>) -> bool {
        let mac = self.mac(key);
        self.mac.0 == mac.0
    }

    pub fn update_mac(&mut self, key: &Secret<OpaqueKMacKey>) {
        self.mac = self.mac(key);
        log!(LogFlags::Info, "MAC: {:?}", self.mac);
    }

    fn data_as_bytes(&self) -> &[u8] {
        unsafe {
            core::slice::from_raw_parts(
                &self.data as *const T as *const u8,
                core::mem::size_of::<T>(),
            )
        }
    }
}
