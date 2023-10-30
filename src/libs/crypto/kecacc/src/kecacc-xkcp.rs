/*
 * Copyright (C) 2021, Stephan Gerhold <stephan.gerhold@mailbox.tu-dresden.de>
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
use base::crypto::HashType;
use base::mem;

const STATE_SIZE64: usize = 256 / mem::size_of::<u64>();

/// Represents the state of the KecAcc accelerator.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct KecAccState {
    data: [u64; STATE_SIZE64],
}

impl KecAccState {
    pub const fn new() -> Self {
        Self {
            data: [0u64; STATE_SIZE64],
        }
    }

    fn as_mut_ptr(&mut self) -> *mut KecAccState {
        self as *mut KecAccState
    }
}

/// A simple wrapper around an emulated Keccak/SHA-3 accelerator ("KecAcc").
/// Note that unlike the version in kecacc.rs this version does not actually
/// make use of an accelerator. Instead, all the hash calculations are done
/// synchronously on the CPU (using the same backend as used in gem5).
/// This is only intended as fallback when running without gem5 or when the
/// accelerator is missing.
pub struct KecAcc {
    state: StaticRefCell<KecAccState>,
}

extern "C" {
    // This is implemented by libkecacc-xkcp with the same backend code as used in gem5
    fn kecacc_init(s: *mut KecAccState, hash_type: u8) -> bool;
    fn kecacc_absorb(s: *mut KecAccState, buf: *const u8, num_bytes: usize) -> usize;
    fn kecacc_squeeze(s: *mut KecAccState, buf: *mut u8, num_bytes: usize) -> usize;
    fn kecacc_pad(s: *mut KecAccState);
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

    pub fn poll_complete_barrier(&self) {
        self.poll_complete()
        // No need for memory barrier because no hardware is involved
    }

    pub fn start_init(&self, hash_type: HashType) {
        unsafe {
            assert!(kecacc_init(
                self.state.borrow_mut().as_mut_ptr(),
                hash_type as u8
            ));
        }
    }

    pub fn start_load(&self, state: &KecAccState) {
        self.state.borrow_mut().data.copy_from_slice(&state.data);
    }

    pub fn start_save(&self, state: &mut KecAccState) {
        state.data.copy_from_slice(&self.state.borrow().data);
    }

    pub fn start_absorb(&self, buf: &[u8]) {
        unsafe {
            kecacc_absorb(
                self.state.borrow_mut().as_mut_ptr(),
                buf.as_ptr(),
                buf.len(),
            )
        };
    }

    pub fn start_pad(&self) {
        unsafe { kecacc_pad(self.state.borrow_mut().as_mut_ptr()) };
    }

    pub fn start_absorb_last(&self, buf: &[u8]) {
        self.start_absorb(buf);
        self.start_pad();
    }

    pub fn start_squeeze(&self, buf: &mut [u8]) {
        unsafe {
            kecacc_squeeze(
                self.state.borrow_mut().as_mut_ptr(),
                buf.as_mut_ptr(),
                buf.len(),
            )
        };
    }
}
