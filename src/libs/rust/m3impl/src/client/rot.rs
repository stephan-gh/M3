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

use crate::client::ClientSession;
use crate::com::{opcodes, MemGate, RecvGate, SendGate};
use crate::errors::{Code, Error};
use crate::mem::GlobOff;
use crate::serialize::bytes::{ByteBuf, Bytes};
use crate::vec::Vec;

pub struct RoTSession {
    sess: ClientSession,
    sgate: SendGate,
    secret_mem: MemGate,
}

impl RoTSession {
    pub fn new(name: &str) -> Result<Self, Error> {
        let sess = ClientSession::new(name)?;
        let sgate = sess.connect()?;
        let secret_mem = sess.obtain(1, |is| is.push(opcodes::RoT::GetSecretMem), |_| Ok(()))?;

        Ok(RoTSession {
            sess,
            sgate,
            secret_mem: MemGate::new_bind(secret_mem.start())?,
        })
    }

    pub fn read_rot_certificate(&self) -> Result<Vec<u8>, Error> {
        let mut off = 0;
        let mut size = 0;
        let mem = self.sess.obtain(
            1,
            |is| is.push(opcodes::RoT::GetRotCertificate),
            |os| {
                (off, size) = os.pop()?;
                Ok(())
            },
        )?;
        let mgate = MemGate::new_bind(mem.start())?;
        mgate.read_into_vec(size, off)
    }

    pub fn get_hash(&self) -> Result<Vec<u8>, Error> {
        Ok(
            send_recv_res!(self.sgate, RecvGate::def(), opcodes::RoT::GetHash)?
                .pop::<ByteBuf>()?
                .into_vec(),
        )
    }

    pub fn secret_mem(&self) -> &MemGate {
        &self.secret_mem
    }

    pub fn get_cdi(&self) -> Result<(GlobOff, usize), Error> {
        send_recv_res!(self.sgate, RecvGate::def(), opcodes::RoT::GetCdi)?.pop()
    }

    pub fn read_cdi<const N: usize>(&self) -> Result<[u8; N], Error> {
        let (off, size) = self.get_cdi()?;
        assert_eq!(size, N);
        self.secret_mem.read_obj(off)
    }

    pub fn derive_secret(&self, custom: &str, size: usize) -> Result<GlobOff, Error> {
        let (off, derived_size): (GlobOff, usize) = send_recv_res!(
            self.sgate,
            RecvGate::def(),
            opcodes::RoT::DeriveSecret,
            custom,
            size
        )?
        .pop()?;
        assert_eq!(derived_size, size);
        Ok(off)
    }

    pub fn read_derived_secret<const N: usize>(&self, custom: &str) -> Result<[u8; N], Error> {
        let off = self.derive_secret(custom, N)?;
        self.secret_mem.read_obj(off)
    }

    pub fn sign<const N: usize>(&self, bytes: &[u8]) -> Result<[u8; N], Error> {
        send_recv_res!(
            self.sgate,
            RecvGate::def(),
            opcodes::RoT::Certify,
            Bytes::new(bytes)
        )?
        .pop::<&[u8]>()?
        .try_into()
        .map_err(|_| Error::new(Code::InvArgs))
    }
}
