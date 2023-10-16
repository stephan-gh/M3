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

use m3::io::LogFlags;
use m3::log;
use rot::cert::{PayloadPubKey, SignaturePayload};
use rot::ed25519::{Signature, SignatureError, VerifyingKey, PUBLIC_KEY_LENGTH};

pub trait CheckSignature {
    fn check_signature(&self) -> Result<(), SignatureError>;
    fn check_all_signatures(&self) -> Result<(), SignatureError>;
}

impl CheckSignature for () {
    fn check_signature(&self) -> Result<(), SignatureError> {
        Ok(())
    }

    fn check_all_signatures(&self) -> Result<(), SignatureError> {
        Ok(())
    }
}

impl<T: SignaturePayload, P: CheckSignature> CheckSignature for rot::cert::Certificate<T, P> {
    fn check_signature(&self) -> Result<(), SignatureError> {
        let pub_key = VerifyingKey::from_bytes(&self.pub_key.0)?;
        let signature = Signature::from_bytes(&self.signature);
        let payload = self.payload.as_bytes();

        log!(
            LogFlags::Info,
            "Checking {} signature from {}",
            core::any::type_name::<T>(),
            self.pub_key
        );
        pub_key.verify_strict(payload, &signature).inspect_err(|e| {
            log!(
                LogFlags::Error,
                "Signature from {} failed to verify: {}",
                self.pub_key,
                e,
            )
        })
    }

    fn check_all_signatures(&self) -> Result<(), SignatureError> {
        self.check_signature()?;
        self.parent.check_all_signatures()
    }
}

pub trait CheckCertificateLink {
    fn check_certificate_link(&self, pub_key: &[u8; PUBLIC_KEY_LENGTH]) -> bool;
}

pub trait CheckCertificateChain {
    fn check_certificate_chain(&self) -> bool;
}

impl CheckCertificateLink for () {
    fn check_certificate_link(&self, _pub_key: &[u8; PUBLIC_KEY_LENGTH]) -> bool {
        true
    }
}

impl<T: PayloadPubKey, P: CheckCertificateLink> CheckCertificateLink
    for rot::cert::Certificate<T, P>
{
    fn check_certificate_link(&self, pub_key: &[u8; PUBLIC_KEY_LENGTH]) -> bool {
        *self.payload.pub_key() == *pub_key && self.parent.check_certificate_link(&self.pub_key.0)
    }
}

impl<T, P: CheckCertificateLink> CheckCertificateChain for rot::cert::Certificate<T, P> {
    fn check_certificate_chain(&self) -> bool {
        self.parent.check_certificate_link(&self.pub_key.0)
    }
}
