use crate::com::{MemGate, RecvGate, SendGate};
use crate::errors::Error;
use crate::int_enum;
use crate::kif::{CapRngDesc, CapType};
use crate::pes::VPE;
use crate::serialize::Sink;
use crate::session::ClientSession;
use crate::{goff, math};

use core::{cmp, fmt};

pub const MSG_SIZE: usize = 128;
pub const MSG_SLOTS: usize = 1;

pub type BlockNo = u32;

#[derive(Copy, Clone, PartialOrd, PartialEq, Eq)]
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
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}..{}", self.start, self.start + self.count - 1)
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
        let crd = CapRngDesc::new(CapType::OBJECT, VPE::cur().alloc_sel(), 1);
        sess.obtain_for(
            VPE::cur().sel(),
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
                slice_sink.push(&blocks.start);
                slice_sink.push(&blocks.count);
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
