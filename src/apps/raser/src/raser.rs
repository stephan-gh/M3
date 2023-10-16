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

use m3::client::RoTSession;
use m3::errors::Error;
use m3::io::LogFlags;
use m3::vfs::VFS;
use m3::{log, println};
use rot::ed25519::{SecretKey, SigningKey};
use rot::Secret;

mod cert;
mod nets;
mod rotc;

#[no_mangle]
pub fn main() -> Result<(), Error> {
    log!(LogFlags::Info, "Hello World!");
    if let Err(e) = VFS::mount("/", "m3fs", "m3fs") {
        log!(LogFlags::Error, "Cannot mount file system: {}", e);
    }

    let rot = RoTSession::new("rot").expect("failed to open RoT session");
    log!(LogFlags::Info, "Asking RoTS to derive secret key");
    let seed: Secret<SecretKey> = Secret::new(
        rot.read_derived_secret("ED25519")
            .expect("Failed to derive ed25519 key"),
    );
    let sig_key = SigningKey::from_bytes(&seed.secret);
    log!(LogFlags::RoTDbg, "Derived own {:?}", sig_key);

    let cert = rotc::obtain_certificate(&rot, &sig_key)?;
    println!("{}", rot::json::to_string_pretty(&cert).unwrap());
    nets::serve(&sig_key, &cert)
}
