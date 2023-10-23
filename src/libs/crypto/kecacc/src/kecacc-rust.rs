/*
 * Copyright (C) 2023, Stephan Gerhold <stephan.gerhold@mailbox.tu-dresden.de>
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

use base::cell::StaticRefCell;
use base::mem;
use sha3::digest::{ExtendableOutput, FixedOutput, Output, Update, XofReader};
use Sha3State::*;

#[derive(Clone)]
enum Sha3State {
    Reset,
    Sha3_224(sha3::Sha3_224),
    Sha3_224Output(Output<sha3::Sha3_224>),
    Sha3_256(sha3::Sha3_256),
    Sha3_256Output(Output<sha3::Sha3_256>),
    Sha3_384(sha3::Sha3_384),
    Sha3_384Output(Output<sha3::Sha3_384>),
    Sha3_512(sha3::Sha3_512),
    Sha3_512Output(Output<sha3::Sha3_512>),
    Shake128(sha3::Shake128),
    Shake128Squeeze(sha3::Shake128Reader),
    Shake256(sha3::Shake256),
    Shake256Squeeze(sha3::Shake256Reader),
}

/// Represents the state of the KecAcc accelerator.
#[derive(Clone)]
pub struct KecAccState {
    state: Sha3State,
}

impl KecAccState {
    pub const fn new() -> Self {
        Self { state: Reset }
    }
}

/// A simple wrapper around an emulated Keccak/SHA-3 accelerator ("KecAcc").
/// Note that unlike the version in kecacc.rs this version does not actually
/// make use of an accelerator. Instead, all the hash calculations are done
/// synchronously on the CPU (here using the "sha3" Rust crate). This is only
/// intended as fallback when running without gem5 or when the accelerator
/// is missing.
///
/// NOTE: Since this is implemented on top of the "sha3" crate, it behaves
/// slightly different from the version in kecacc.rs and kecacc-xkcp.rs.
/// There are additional validation checks enforced by the sha3 crate
/// (e.g. absorb not possible after squeeze). Also, the state is reset
/// after squeeze has been called for the fixed-length variants.
pub struct KecAcc {
    state: StaticRefCell<KecAccState>,
}

impl KecAcc {
    pub const fn new(_addr: usize) -> Self {
        KecAcc {
            state: StaticRefCell::new(KecAccState::new()),
        }
    }

    pub fn is_busy(&self) -> bool {
        false
    }

    pub fn poll_complete(&self) {
        while self.is_busy() {}
    }

    pub fn start_init(&self, hash_type: u8) {
        let mut s = self.state.borrow_mut();
        match hash_type {
            0x0 => s.state = Reset,
            0x1 => s.state = Sha3_224(sha3::Sha3_224::default()),
            0x2 => s.state = Sha3_256(sha3::Sha3_256::default()),
            0x3 => s.state = Sha3_384(sha3::Sha3_384::default()),
            0x4 => s.state = Sha3_512(sha3::Sha3_512::default()),
            0x5 => s.state = Shake128(sha3::Shake128::default()),
            0x6 => s.state = Shake256(sha3::Shake256::default()),
            _ => panic!("Invalid hash type: {}", hash_type),
        }
    }

    pub fn start_load(&self, state: &KecAccState) {
        let mut s = self.state.borrow_mut();
        s.state = state.state.clone();
    }

    pub fn start_save(&self, state: &mut KecAccState) {
        let s = self.state.borrow();
        state.state = s.state.clone();
    }

    pub fn start_absorb(&self, buf: &[u8]) {
        let mut s = self.state.borrow_mut();
        match &mut s.state {
            Sha3_224(sha) => sha.update(buf),
            Sha3_256(sha) => sha.update(buf),
            Sha3_384(sha) => sha.update(buf),
            Sha3_512(sha) => sha.update(buf),
            Shake128(sha) => sha.update(buf),
            Shake256(sha) => sha.update(buf),
            Reset => panic!("KecAcc not initialized"),
            _ => panic!("Cannot absorb after starting squeeze"),
        }
    }

    pub fn start_pad(&self) {
        let mut s = self.state.borrow_mut();
        match mem::replace(&mut s.state, Reset) {
            Sha3_224(sha) => s.state = Sha3_224Output(sha.finalize_fixed()),
            Sha3_256(sha) => s.state = Sha3_256Output(sha.finalize_fixed()),
            Sha3_384(sha) => s.state = Sha3_384Output(sha.finalize_fixed()),
            Sha3_512(sha) => s.state = Sha3_512Output(sha.finalize_fixed()),
            Shake128(sha) => s.state = Shake128Squeeze(sha.finalize_xof()),
            Shake256(sha) => s.state = Shake256Squeeze(sha.finalize_xof()),
            Reset => panic!("KecAcc not initialized"),
            _ => panic!("Cannot pad after starting squeeze"),
        };
    }

    pub fn start_squeeze(&self, buf: &mut [u8]) {
        let mut s = self.state.borrow_mut();
        match &mut s.state {
            Sha3_224Output(out) => {
                buf.copy_from_slice(&out);
                s.state = Reset;
            },
            Sha3_256Output(out) => {
                buf.copy_from_slice(&out);
                s.state = Reset;
            },
            Sha3_384Output(out) => {
                buf.copy_from_slice(&out);
                s.state = Reset;
            },
            Sha3_512Output(out) => {
                buf.copy_from_slice(&out);
                s.state = Reset;
            },
            Shake128Squeeze(s) => s.read(buf),
            Shake256Squeeze(s) => s.read(buf),
            Reset => panic!("KecAcc not initialized"),
            _ => panic!("Cannot squeeze before padding"),
        };
    }
}
