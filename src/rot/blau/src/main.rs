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

#![no_std]
#![no_main]

use riscv_rt::entry;

use base::io::log::LogColor;
use base::io::{log, LogFlags};
use base::{env, log, machine};
#[allow(unused_imports)]
use lang as _;
use rot::cert::{BinaryPayload, SignaturePayload};
use rot::ed25519::{SecretKey, Signer, SigningKey};
use rot::{Hex, Secret};

#[no_mangle]
pub extern "C" fn exit(_code: i32) -> ! {
    log!(LogFlags::Info, "Shutting down");
    machine::shutdown();
}

#[entry]
fn main() -> ! {
    log::init(env::boot().tile_id(), "blau", LogColor::BrightBlue);
    log!(LogFlags::RoTBoot, "Hello World");

    let ctx = unsafe { rot::BromLayerCtx::take() };
    let cfg = unsafe { rot::BlauLayerCfg::get() };

    // Load binary for next layer and derive CDI
    let next = unsafe { rot::load_bin(rot::BLAU_NEXT_ADDR, &cfg.data.next_layer) };
    let mut next_cdi = Secret::new_zeroed();
    rot::derive_cdi(&ctx.data.kmac_cdi, next, &mut next_cdi);

    // Derive signing key used by next layer
    let mut next_seed: Secret<SecretKey> = Secret::new_zeroed();
    rot::derive_key(&next_cdi, "ED25519", &[], &mut next_seed.secret[..]);
    let cached = cfg.data.cache.check(&ctx.data.kmac_cdi) && cfg.data.next_cache.check(&next_cdi);
    if !cached {
        let next_sig_key = SigningKey::from_bytes(&next_seed.secret);
        log!(LogFlags::RoTDbg, "Derived next layer {:?}", next_sig_key);
        cfg.data.next_cache.data.pub_key = Hex(next_sig_key.verifying_key().to_bytes());
    }

    // Prepare signature payload by hashing next layer again
    let mut payload = BinaryPayload {
        hash: Hex::new_zeroed(),
        pub_key: Hex(cfg.data.next_cache.data.pub_key.0),
    };
    rot::hash(rot::cert::HASH_TYPE, next, &mut payload.hash[..]);
    log!(LogFlags::RoTBoot, "{:#?}", payload);

    if !cached {
        // Derive own signing key
        let mut seed: Secret<SecretKey> = Secret::new_zeroed();
        rot::derive_key(&ctx.data.kmac_cdi, "ED25519", &[], &mut seed.secret[..]);
        let sig_key = SigningKey::from_bytes(&seed.secret);
        log!(LogFlags::RoTDbg, "Derived own {:?}", sig_key);

        // Create signature
        let signature = Hex(sig_key.sign(payload.as_bytes()).to_bytes());
        log!(LogFlags::RoTDbg, "Signed: {}", signature);

        cfg.data.cache.data = Hex(sig_key.verifying_key().to_bytes());
        cfg.data.next_cache.data.signature = signature;

        cfg.data.cache.update_mac(&ctx.data.kmac_cdi);
        cfg.data.next_cache.update_mac(&next_cdi);
    };
    log!(LogFlags::Info, "Verification key: {}", cfg.data.cache.data);

    // Switch to next layer
    let next_ctx = rot::LayerCtx::new(rot::BLAU_NEXT_ADDR, rot::BlauCtx {
        kmac_cdi: next_cdi,
        derived_private_key: next_seed,
        signer_public_key: Hex(cfg.data.cache.data.0),
        signature: Hex(cfg.data.next_cache.data.signature.0),
        signed_payload: payload,
    });
    unsafe { next_ctx.switch() }
}
