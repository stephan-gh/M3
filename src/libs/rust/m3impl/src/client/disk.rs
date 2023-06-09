/*
 * Copyright (C) 2020-2022 Nils Asmussen, Barkhausen Institut
 * Copyright (C) 2019-2020, Tendsin Mende <tendsin@protonmail.com>
 * Copyright (C) 2018, Sebastian Reimers <sebastian.reimers@mailbox.tu-dresden.de>
 *
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
use crate::errors::Error;
use crate::kif::{CapRngDesc, CapType};
use crate::mem::GlobOff;
use crate::util::math;

use core::{cmp, fmt};

pub const MSG_SIZE: usize = 128;
pub const MSG_SLOTS: usize = 1;

pub type DiskBlockNo = u32;

/// A range of blocks on a hard disk
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct DiskBlockRange {
    pub start: DiskBlockNo,
    pub count: DiskBlockNo,
}

impl DiskBlockRange {
    /// Creates a `DiskBlockRange` only for the given block
    pub fn new(bno: DiskBlockNo) -> Self {
        Self::new_range(bno, 1)
    }

    /// Creates a `DiskBlockRange` for the given range: `start`..`start`+`count` (not including
    /// block `start`+`count`)
    pub fn new_range(start: DiskBlockNo, count: DiskBlockNo) -> Self {
        DiskBlockRange { start, count }
    }
}

impl fmt::Debug for DiskBlockRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.start, self.start + self.count - 1)
    }
}

impl cmp::PartialOrd for DiskBlockRange {
    fn partial_cmp(&self, other: &DiskBlockRange) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl cmp::Ord for DiskBlockRange {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        if self.start >= other.start && self.start < other.start + other.count {
            cmp::Ordering::Equal
        }
        else if self.start < other.start {
            cmp::Ordering::Less
        }
        else {
            cmp::Ordering::Greater
        }
    }
}

/// Represents a session at the disk server
pub struct Disk {
    sess: ClientSession,
    rgate: RecvGate,
    sgate: SendGate,
}

impl Disk {
    pub fn new(name: &str) -> Result<Self, Error> {
        // connect to disk service
        let sess = ClientSession::new(name)?;

        // create receive gate for the responses
        let rgate = RecvGate::new(
            math::next_log2(MSG_SIZE * MSG_SLOTS),
            math::next_log2(MSG_SIZE),
        )?;
        rgate.activate()?;

        // get send gate for our requests
        let sgate = sess.connect()?;

        Ok(Disk { sess, rgate, sgate })
    }

    pub fn delegate_mem(&self, mem: &MemGate, blocks: DiskBlockRange) -> Result<(), Error> {
        let crd = CapRngDesc::new(CapType::Object, mem.sel(), 1);
        self.sess.delegate(
            crd,
            |slice_sink| {
                slice_sink.push(opcodes::Disk::AddMem);
                slice_sink.push(blocks.start);
                slice_sink.push(blocks.count);
            },
            |_slice_source| Ok(()),
        )
    }

    pub fn read(
        &self,
        cap: DiskBlockNo,
        blocks: DiskBlockRange,
        blocksize: usize,
        off: Option<GlobOff>,
    ) -> Result<(), Error> {
        send_recv_res!(
            &self.sgate,
            &self.rgate,
            opcodes::Disk::Read,
            cap,
            blocks.start,
            blocks.count,
            blocksize,
            off.unwrap_or(0)
        )
        .map(|_| ())
    }

    pub fn write(
        &self,
        cap: DiskBlockNo,
        blocks: DiskBlockRange,
        blocksize: usize,
        off: Option<GlobOff>,
    ) -> Result<(), Error> {
        send_recv_res!(
            &self.sgate,
            &self.rgate,
            opcodes::Disk::Write,
            cap,
            blocks.start,
            blocks.count,
            blocksize,
            off.unwrap_or(0)
        )
        .map(|_| ())
    }
}
