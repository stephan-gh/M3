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

use crate::com::{MemGate, RecvGate, SendGate};
use crate::errors::Error;
use crate::int_enum;
use crate::kif::{CapRngDesc, CapType};
use crate::session::ClientSession;
use crate::tiles::Activity;
use crate::{goff, math};

use core::{cmp, fmt};

pub const MSG_SIZE: usize = 128;
pub const MSG_SLOTS: usize = 1;

pub type BlockNo = u32;

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct BlockRange {
    pub start: BlockNo,
    pub count: BlockNo,
}

impl BlockRange {
    pub fn new(bno: BlockNo) -> Self {
        Self::new_range(bno, 1)
    }

    pub fn new_range(start: BlockNo, count: BlockNo) -> Self {
        BlockRange { start, count }
    }
}

impl fmt::Debug for BlockRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.start, self.start + self.count - 1)
    }
}

impl cmp::PartialOrd for BlockRange {
    fn partial_cmp(&self, other: &BlockRange) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl cmp::Ord for BlockRange {
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

int_enum! {
    pub struct DiskOperation : u32 {
        const READ  = 0x0;
        const WRITE = 0x1;
    }
}

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
        let mut rgate = RecvGate::new(
            math::next_log2(MSG_SIZE * MSG_SLOTS),
            math::next_log2(MSG_SIZE),
        )?;
        rgate.activate()?;

        // get send gate for our requests
        let crd = CapRngDesc::new(CapType::OBJECT, Activity::own().alloc_sel(), 1);
        sess.obtain_for(
            Activity::own().sel(),
            crd,
            |_slice_sink| {},
            |_slice_source| Ok(()),
        )?;
        let sgate = SendGate::new_bind(crd.start());

        Ok(Disk { sess, rgate, sgate })
    }

    pub fn delegate_mem(&self, mem: &MemGate, blocks: BlockRange) -> Result<(), Error> {
        let crd = CapRngDesc::new(CapType::OBJECT, mem.sel(), 1);
        self.sess.delegate(
            crd,
            |slice_sink| {
                slice_sink.push_word(blocks.start as u64);
                slice_sink.push_word(blocks.count as u64);
            },
            |_slice_source| Ok(()),
        )
    }

    pub fn read(
        &self,
        cap: BlockNo,
        blocks: BlockRange,
        blocksize: usize,
        off: Option<goff>,
    ) -> Result<(), Error> {
        if let Err(e) = send_recv_res!(
            &self.sgate,
            &self.rgate,
            DiskOperation::READ.val,
            cap,
            blocks.start,
            blocks.count,
            blocksize,
            off.unwrap_or(0)
        ) {
            Err(e)
        }
        else {
            Ok(())
        }
    }

    pub fn write(
        &self,
        cap: BlockNo,
        blocks: BlockRange,
        blocksize: usize,
        off: Option<goff>,
    ) -> Result<(), Error> {
        if let Err(e) = send_recv_res!(
            &self.sgate,
            &self.rgate,
            DiskOperation::WRITE.val,
            cap,
            blocks.start,
            blocks.count,
            blocksize,
            off.unwrap_or(0)
        ) {
            Err(e)
        }
        else {
            Ok(())
        }
    }
}
