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

use base::io::LogFlags;
use base::{cfg, log};
use core::fmt::Debug;

use crate::cert::BinaryPayload;
use crate::{ed25519, encode_magic, Hex, Magic, OpaqueKMacKey, Secret};

pub trait CtxData: Debug {
    const MAGIC: Magic;

    fn check_magic(magic: Magic) {
        assert_eq!(magic, Self::MAGIC);
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct BromCtx {
    pub kmac_cdi: Secret<OpaqueKMacKey>,
}

impl CtxData for BromCtx {
    const MAGIC: Magic = encode_magic(b"BromCtx", 1);
}

#[repr(C)]
#[derive(Debug)]
pub struct BlauCtx {
    pub kmac_cdi: Secret<OpaqueKMacKey>,
    pub derived_private_key: Secret<ed25519::SecretKey>,
    pub signer_public_key: Hex<[u8; ed25519::PUBLIC_KEY_LENGTH]>,
    pub signature: Hex<[u8; ed25519::SIGNATURE_LENGTH]>,
    pub signed_payload: BinaryPayload,
}

impl CtxData for BlauCtx {
    const MAGIC: Magic = encode_magic(b"BlauCtx", 1);
}

#[repr(C)]
#[derive(Debug)]
pub struct RosaCtx {
    pub kmac_cdi: Secret<OpaqueKMacKey>,
    pub derived_private_key: Secret<ed25519::SecretKey>,
    // rot-certificate.json is stored in DRAM as regular boot module
}

impl CtxData for RosaCtx {
    const MAGIC: Magic = encode_magic(b"RosaCtx", 1);
}

#[repr(C)]
#[derive(Debug)]
pub struct LayerCtx<Data: CtxData> {
    pub brom_hdr_magic: Magic,
    pub entry_addr: u64,
    pub magic: Magic,
    pub data: Data,
}

pub type BromLayerCtx = LayerCtx<BromCtx>;
pub type BlauLayerCtx = LayerCtx<BlauCtx>;
pub type RosaLayerCtx = LayerCtx<RosaCtx>;

impl CtxData for () {
    const MAGIC: Magic = 0;

    fn check_magic(_magic: Magic) {
        // No data so anything is fine
    }
}

impl<Data: CtxData> LayerCtx<Data> {
    pub const BROM_HDR_MAGIC: Magic = encode_magic(b"BromHdr", 1);
    // Context is placed immediately at start of SRAM
    pub const MEM_OFFSET: usize = cfg::MEM_OFFSET;

    pub fn new(entry_addr: usize, data: Data) -> Self {
        Self {
            brom_hdr_magic: Self::BROM_HDR_MAGIC,
            entry_addr: entry_addr as u64,
            magic: Data::MAGIC,
            data,
        }
    }

    fn check_magic(&self) {
        assert_eq!(self.brom_hdr_magic, Self::BROM_HDR_MAGIC);
        Data::check_magic(self.magic);
    }

    /// Get a reference to the current layer context.
    ///
    /// # Safety
    /// The caller must ensure that the context is accessible at
    /// `Self::MEM_OFFSET`. This is generally only the case for the RoT tile
    /// where the Boot ROM or previous layers have initialized the context.
    pub unsafe fn get() -> &'static mut Self {
        let ctx = Self::MEM_OFFSET as *mut Self;
        let ctx = unsafe { &mut *ctx };
        ctx.check_magic();
        ctx
    }

    /// Get a reference to the current layer context.
    ///
    /// # Safety
    /// The caller must ensure that the context is accessible at
    /// `Self::MEM_OFFSET`. This is generally only the case for the RoT tile
    /// where the Boot ROM or previous layers have initialized the context.
    /// The stack must be large enough to not grow into the context.
    pub unsafe fn take() -> Self {
        let ctx = Self::MEM_OFFSET as *mut Self;
        let copy = ctx.read();
        // Zero out the original context for extra hardening
        base::util::clear_volatile(ctx);
        copy.check_magic();
        log!(LogFlags::RoTDbg, "{:#x?}", copy);
        copy
    }

    /// Switch to the next layer, at the specified entry address.
    /// This will:
    ///   - Copy the context to `Self::MEM_OFFSET`
    ///   - Clear the rest of the stack and the BSS so that no secrets are
    ///     leaked into the next (potentially untrusted) boot layer.
    ///
    /// # Safety
    /// The entry address must be valid and not cleared as part of the cleanup.
    #[cfg(target_arch = "riscv64")]
    pub unsafe fn switch(self) -> ! {
        crate::asm::switch(self);
    }

    /// Go to sleep, waiting for an external entity to trigger a CPU reset
    /// that will end up booting into the next layer at the specified
    /// entry_addr.
    ///
    /// # Safety
    /// The entry address must be valid when the reset is triggered externally.
    ///
    /// **NOTE:** This function does NOT perform a context switch. Secrets
    /// (if any) should be erased before calling sleep().
    #[cfg(target_arch = "riscv64")]
    pub unsafe fn sleep(&self) -> ! {
        crate::asm::sleep(self);
    }
}
