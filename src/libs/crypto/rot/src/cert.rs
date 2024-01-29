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

use core::fmt::Debug;

use serde_json::value::RawValue;

use base::boxed::Box;
use base::col::{BTreeMap, Vec};
use base::crypto::{HashAlgorithm, HashType};
use base::kif::TileDesc;
use base::mem::GlobOff;
use base::serialize::{Deserialize, Serialize};

use crate::ed25519;
use crate::hex::Hex;

pub const HASH_ALGO: &HashAlgorithm = &HashAlgorithm::SHA3_256;
pub const HASH_TYPE: HashType = HASH_ALGO.ty;

pub type HashBuf = [u8; HASH_ALGO.output_bytes];

#[derive(Serialize, Deserialize, Debug)]
#[serde(crate = "base::serde")]
pub struct Certificate<T, P> {
    pub payload: T,
    pub signature: Hex<[u8; ed25519::SIGNATURE_LENGTH]>,
    pub pub_key: Hex<[u8; ed25519::PUBLIC_KEY_LENGTH]>,
    pub parent: P,
}

#[repr(C)]
#[derive(Serialize, Deserialize, Debug)]
#[serde(crate = "base::serde", tag = "type", rename = "binary")]
pub struct BinaryPayload {
    pub hash: Hex<HashBuf>,
    pub pub_key: Hex<[u8; ed25519::PUBLIC_KEY_LENGTH]>,
}

impl BinaryPayload {
    const SIZE: usize = core::mem::size_of::<BinaryPayload>();
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(crate = "base::serde")]
pub struct M3KernelConfig<'a> {
    pub mem_size: GlobOff,
    pub eps_num: u32,
    pub cmdline: &'a str,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(crate = "base::serde", tag = "type", rename = "m3")]
pub struct M3Payload<'a> {
    pub tiles: Vec<TileDesc>,
    #[serde(borrow)]
    pub kernel: M3KernelConfig<'a>,
    pub mods: BTreeMap<&'a str, Hex<HashBuf>>,
    pub pub_key: Hex<[u8; ed25519::PUBLIC_KEY_LENGTH]>,
}

pub trait SignaturePayload {
    fn as_bytes(&self) -> &[u8];
}

impl SignaturePayload for Box<RawValue> {
    fn as_bytes(&self) -> &[u8] {
        self.get().as_bytes()
    }
}

impl SignaturePayload for BinaryPayload {
    fn as_bytes(&self) -> &[u8] {
        unsafe {
            core::slice::from_raw_parts(self as *const BinaryPayload as *const u8, Self::SIZE)
        }
    }
}

pub trait PayloadPubKey {
    fn pub_key(&self) -> &[u8; ed25519::PUBLIC_KEY_LENGTH];
}

impl PayloadPubKey for BinaryPayload {
    fn pub_key(&self) -> &[u8; ed25519::PUBLIC_KEY_LENGTH] {
        &self.pub_key.0
    }
}

impl<'a> PayloadPubKey for M3Payload<'a> {
    fn pub_key(&self) -> &[u8; ed25519::PUBLIC_KEY_LENGTH] {
        &self.pub_key.0
    }
}

pub type M3Certificate<'a> = Certificate<M3Payload<'a>, Certificate<BinaryPayload, ()>>;
pub type M3RawCertificate = Certificate<Box<RawValue>, Certificate<BinaryPayload, ()>>;
