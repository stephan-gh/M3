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

use m3::client::HashSession;
use m3::com::{MemGate, Perm};
use m3::crypto::HashAlgorithm;
use m3::mem::GlobOff;
use m3::wv_assert_ok;

pub fn prepare_shake_mem(size: usize) -> MemGate {
    let mgate = wv_assert_ok!(MemGate::new(size as GlobOff, Perm::RW));
    let hash = wv_assert_ok!(HashSession::new("hash-prepare", &HashAlgorithm::SHAKE128));

    // Fill memory with pseudo-random data from SHAKE128
    let mgated = wv_assert_ok!(mgate.derive_cap(0, size as GlobOff, Perm::W));
    wv_assert_ok!(hash.ep().configure(mgated.sel()));
    wv_assert_ok!(hash.output(0, size));
    mgate
}
