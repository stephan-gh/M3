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

use core::ops::Deref;
use m3::cell::{LazyReadOnlyCell, StaticRefCell};
use m3::com::{opcodes, GateIStream, MemCap, Perm};
use m3::errors::{Code, Error};
use m3::io::LogFlags;
use m3::kif::{CapRngDesc, CapType};
use m3::mem::{GlobOff, VirtAddr};
use m3::serialize::bytes::Bytes;
use m3::server::{
    CapExchange, ClientManager, ExcType, RequestHandler, RequestSession, Server, ServerSession,
    SessId,
};
use m3::tiles::Activity;
use m3::{log, mem, reply_vmsg};
use rot::ed25519::Signer;
use rot::{ed25519, Hex, OpaqueKMacKey, Secret};

const MAX_MSG_SIZE: usize = 256;

const MAX_DERIVED_SECRET_SIZE: usize = 64;
const SECRET_AREA_SIZE: usize = 4096;
const MAX_CLIENTS: usize = SECRET_AREA_SIZE / ClientSecretArea::SIZE;

const HASH_SIZE: usize = rot::cert::HASH_ALGO.output_bytes;
const MAX_SIGN_SIZE: usize = HASH_SIZE + MAX_MSG_SIZE;

static CTX: LazyReadOnlyCell<RotsCtx> = LazyReadOnlyCell::default();
static SECRETS: StaticRefCell<SecretArea> = StaticRefCell::new(SecretArea::new_zeroed());

struct RotsCtx {
    kmac_cdi: Secret<OpaqueKMacKey>,
    signing_key: ed25519::SigningKey,
    rot_cert_cap: MemCap,
    rot_cert_size: GlobOff,
    secret_cap: MemCap,
}

struct ClientSecretArea {
    kmac_cdi: Secret<OpaqueKMacKey>,
    derived_secret: Secret<[u8; MAX_DERIVED_SECRET_SIZE]>,
}

#[repr(align(4096))]
struct SecretArea([ClientSecretArea; MAX_CLIENTS]);

struct RoTSession {
    serv: ServerSession,
    rot_sig_cap: MemCap,
    arg_hash: Hex<[u8; HASH_SIZE]>,
    secret_cap: MemCap,
}

impl ClientSecretArea {
    const SIZE: usize = mem::size_of::<Self>();
    const ZEROED: Self = Self {
        kmac_cdi: Secret::new_zeroed(),
        derived_secret: Secret::new_zeroed(),
    };

    fn clear(&mut self) {
        unsafe {
            m3::util::clear_volatile(self as *mut Self);
        }
    }
}

impl SecretArea {
    const fn new_zeroed() -> Self {
        Self([ClientSecretArea::ZEROED; MAX_CLIENTS])
    }

    fn get(&mut self, sid: SessId) -> &mut ClientSecretArea {
        &mut self.0[sid]
    }

    fn offset(&self, sid: SessId) -> GlobOff {
        (&self.0[sid] as *const _ as usize - self as *const _ as usize) as GlobOff
    }
}

impl Drop for RoTSession {
    fn drop(&mut self) {
        SECRETS.borrow_mut().get(self.sid()).clear();
    }
}

impl RequestSession for RoTSession {
    fn new(serv: ServerSession, arg: &str) -> Result<Self, Error> {
        let sid = serv.id();
        log!(LogFlags::RoTReqs, "[{}] rot::new()", sid);
        let ctx = CTX.get();
        let rot_sig_cap = ctx.rot_cert_cap.derive(0, ctx.rot_cert_size, Perm::R)?;
        let mut secrets = SECRETS.borrow_mut();
        let mut sess = Self {
            serv,
            rot_sig_cap,
            arg_hash: Hex::new_zeroed(),
            secret_cap: ctx.secret_cap.derive(
                secrets.offset(sid),
                ClientSecretArea::SIZE as GlobOff,
                Perm::R,
            )?,
        };
        let csecrets = secrets.get(sid);
        rot::derive_cdi(&ctx.kmac_cdi, arg.as_bytes(), &mut csecrets.kmac_cdi);
        rot::hash(rot::cert::HASH_TYPE, arg.as_bytes(), &mut sess.arg_hash[..]);
        log!(
            LogFlags::RoTReqs,
            "Hash for client '{}': {}",
            arg,
            sess.arg_hash
        );
        Ok(sess)
    }
}

impl RoTSession {
    fn sid(&self) -> SessId {
        self.serv.id()
    }

    fn get_rot_certificate(
        cli: &mut ClientManager<Self>,
        _crt: usize,
        sid: SessId,
        xchg: &mut CapExchange<'_>,
    ) -> Result<(), Error> {
        log!(LogFlags::RoTReqs, "[{}] rot::get_rot_certificate()", sid);
        let ctx = CTX.get();
        let sess = cli.get(sid).ok_or_else(|| Error::new(Code::InvArgs))?;
        xchg.out_caps(CapRngDesc::new(CapType::Object, sess.rot_sig_cap.sel(), 1));
        xchg.out_args().push(0);
        xchg.out_args().push(ctx.rot_cert_size);
        Ok(())
    }

    fn get_secret_mem(
        cli: &mut ClientManager<Self>,
        _crt: usize,
        sid: SessId,
        xchg: &mut CapExchange<'_>,
    ) -> Result<(), Error> {
        log!(LogFlags::RoTReqs, "[{}] rot::get_secret_mem()", sid);
        let sess = cli.get(sid).ok_or_else(|| Error::new(Code::InvArgs))?;
        xchg.out_caps(CapRngDesc::new(CapType::Object, sess.secret_cap.sel(), 1));
        xchg.out_args().push(0);
        xchg.out_args().push(ClientSecretArea::SIZE);
        Ok(())
    }

    fn get_hash(&mut self, is: &mut GateIStream<'_>) -> Result<(), Error> {
        log!(LogFlags::RoTReqs, "[{}] rot::get_hash()", self.sid());
        reply_vmsg!(is, Code::Success, Bytes::new(&self.arg_hash[..]))
    }

    fn get_cdi(&mut self, is: &mut GateIStream<'_>) -> Result<(), Error> {
        log!(LogFlags::RoTReqs, "[{}] rot::get_cdi()", self.sid());
        reply_vmsg!(
            is,
            Code::Success,
            mem::offset_of!(ClientSecretArea, kmac_cdi),
            mem::size_of::<OpaqueKMacKey>()
        )
    }

    fn derive_secret(&mut self, is: &mut GateIStream<'_>) -> Result<(), Error> {
        log!(LogFlags::RoTReqs, "[{}] rot::derive_secret()", self.sid());
        let custom: &str = is.pop()?;
        let size: usize = is.pop()?;
        if size > MAX_DERIVED_SECRET_SIZE {
            return Err(Error::new(Code::InvArgs));
        }

        let mut secrets = SECRETS.borrow_mut();
        let csecrets = secrets.get(self.sid());
        rot::derive_key(
            &csecrets.kmac_cdi,
            custom,
            &[],
            &mut csecrets.derived_secret.secret[..size],
        );
        reply_vmsg!(
            is,
            Code::Success,
            mem::offset_of!(ClientSecretArea, derived_secret),
            size
        )
    }

    fn certify(&mut self, is: &mut GateIStream<'_>) -> Result<(), Error> {
        log!(LogFlags::RoTReqs, "[{}] rot::certify()", self.sid());

        // Sign requested payload with identity prepended
        let mut buf = [0u8; MAX_SIGN_SIZE];
        buf[0..HASH_SIZE].copy_from_slice(&self.arg_hash[..]);
        let bytes: &[u8] = is.pop()?;
        let end = HASH_SIZE + bytes.len();
        buf[HASH_SIZE..end].copy_from_slice(bytes);

        log!(LogFlags::RoTReqs, "Signing: {}", Hex(&buf[..end]));
        let ctx = CTX.get();
        let signature = ctx.signing_key.sign(&buf[..end]);
        log!(LogFlags::RoTDbg, "Signed: {:x}", signature);

        reply_vmsg!(is, Code::Success, Bytes::new(&signature.to_bytes()[..]))
    }
}

#[no_mangle]
pub fn main() -> Result<(), Error> {
    log!(LogFlags::RoTBoot, "Hello World!");

    {
        let ctx = unsafe { rot::RosaLayerCtx::take() };

        let rot_cert_cap = MemCap::new_bind_bootmod("rot-certificate.json")
            .expect("Failed to get rot-certificate.json boot module");
        let rot_cert_size = rot_cert_cap
            .region()
            .expect("Failed to get rot-certificate.json region")
            .1;

        let secrets = SECRETS.borrow();
        // We don't need the activated MemGate returned by Activity::own().get_mem()
        let secret_cap = MemCap::new_foreign(
            Activity::own().sel(),
            VirtAddr::from(secrets.deref() as *const SecretArea),
            SECRET_AREA_SIZE as GlobOff,
            Perm::R,
        )
        .expect("Failed to get mem cap for secrets");

        CTX.set(RotsCtx {
            kmac_cdi: ctx.data.kmac_cdi,
            signing_key: ed25519::SigningKey::from_bytes(&ctx.data.derived_private_key.secret),
            rot_cert_cap,
            rot_cert_size,
            secret_cap,
        });
    }
    log!(LogFlags::RoTDbg, "Derived own {:?}", CTX.get().signing_key);

    let mut hdl = RequestHandler::new_with(MAX_CLIENTS, MAX_MSG_SIZE, 1)
        .expect("Unable to create request handler");
    let mut srv = Server::new("rot", &mut hdl).expect("Unable to create service 'rot'");

    use opcodes::RoT;
    hdl.reg_cap_handler(
        RoT::GetRotCertificate,
        ExcType::Obt(1),
        RoTSession::get_rot_certificate,
    );
    hdl.reg_cap_handler(
        RoT::GetSecretMem,
        ExcType::Obt(1),
        RoTSession::get_secret_mem,
    );
    hdl.reg_msg_handler(RoT::GetHash, RoTSession::get_hash);
    hdl.reg_msg_handler(RoT::GetCdi, RoTSession::get_cdi);
    hdl.reg_msg_handler(RoT::DeriveSecret, RoTSession::derive_secret);
    hdl.reg_msg_handler(RoT::Certify, RoTSession::certify);

    hdl.run(&mut srv).expect("Server loop failed");
    Ok(())
}
