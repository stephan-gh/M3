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

use crate::cert::{CheckCertificateChain, CheckSignature};
use m3::client::RoTSession;
use m3::errors::{Code, Error};
use m3::io::{LogFlags, Read, Write};
use m3::time::TimeInstant;
use m3::vfs::{OpenFlags, VFS};
use m3::{log, println};
use rot::cert::{BinaryPayload, M3Certificate, M3RawCertificate};
use rot::ed25519::SigningKey;
use rot::Hex;

//pub type RoTCCertificate<'a> = rot::cert::Certificate<BinaryPayload, M3Certificate<'a>>;
pub type RoTCRawCertificate = rot::cert::Certificate<BinaryPayload, M3RawCertificate>;

fn restore_cached_signature(sig: &mut RoTCRawCertificate) -> Result<(), Error> {
    log!(LogFlags::Info, "Loading cached signature from /raser.sig");
    let start = TimeInstant::now();
    VFS::open("/raser.sig", OpenFlags::R)
        .inspect_err(|e| log!(LogFlags::Error, "Failed to open /raser.sig: {}", e))?
        .read_exact(&mut sig.signature[..])
        .inspect_err(|e| log!(LogFlags::Error, "Reading /raser.sig failed: {}", e))?;
    let time = start.elapsed();
    log!(LogFlags::Info, "Loading cached signature took {:?}", time);
    sig.check_signature().map_err(|_| Error::new(Code::InvArgs))
}

fn request_new_signature(
    rot: &RoTSession,
    sig: &mut RoTCRawCertificate,
    pub_key: &[u8],
) -> Result<(), Error> {
    log!(
        LogFlags::Info,
        "Asking RoTS to sign public key '{}'",
        Hex(pub_key)
    );
    let start = TimeInstant::now();
    sig.signature = Hex(rot.sign(pub_key)?);
    let time = start.elapsed();
    log!(LogFlags::Info, "RoTS signature took {:?}", time);
    sig.check_signature().map_err(|_| Error::new(Code::InvArgs))
}

fn store_cached_signature(sig: &RoTCRawCertificate) -> Result<(), Error> {
    log!(LogFlags::Info, "Saving cached signature to /raser.sig");
    let start = TimeInstant::now();
    let mut f = VFS::open("/raser.sig", OpenFlags::W | OpenFlags::CREATE)
        .inspect_err(|e| log!(LogFlags::Error, "Failed to open /raser.sig: {}", e))?;
    f.write(&sig.signature[..])
        .inspect_err(|e| log!(LogFlags::Error, "Failed to write /raser.sig: {}", e))?;
    f.sync()
        .inspect_err(|e| log!(LogFlags::Error, "Failed to sync /raser.sig: {}", e))?;
    let time = start.elapsed();
    log!(LogFlags::Info, "Saving cached signature took {:?}", time);
    Ok(())
}

pub fn obtain_certificate(
    rot: &RoTSession,
    sig_key: &SigningKey,
) -> Result<RoTCRawCertificate, Error> {
    let vec = rot.read_rot_certificate()?;
    let parsed: M3Certificate<'_> =
        rot::json::from_slice(&vec).expect("RoTS provided invalid JSON");
    log!(
        LogFlags::Info,
        "RoTS provided following certificate for itself:"
    );
    println!("{}", rot::json::to_string_pretty(&parsed).unwrap());
    if !parsed.check_certificate_chain() {
        log!(LogFlags::Error, "RoT certificates not correctly chained");
        return Err(Error::new(Code::InvArgs));
    }

    let raw: M3RawCertificate = rot::json::from_slice(&vec).expect("RoTS provided invalid JSON");
    raw.check_all_signatures()
        .map_err(|_| Error::new(Code::InvArgs))?;

    let own_hash = rot.get_hash()?;
    log!(LogFlags::RoTDbg, "Hash: {}", Hex(&own_hash[..]));

    let pub_key = sig_key.verifying_key();
    let mut cert = rot::cert::Certificate {
        payload: BinaryPayload {
            hash: Hex(own_hash.try_into().unwrap()),
            pub_key: Hex(pub_key.to_bytes()),
        },
        signature: Hex::new_zeroed(),
        pub_key: parsed.payload.pub_key,
        parent: raw,
    };
    restore_cached_signature(&mut cert)
        .or_else(|_| request_new_signature(rot, &mut cert, pub_key.as_bytes()))?;
    store_cached_signature(&cert).ok();
    Ok(cert)
}
