use crate::{goff, math};
use crate::com::{RecvGate, SendGate};
use crate::errors::Error;
use crate::session::ClientSession;

pub const MSG_SIZE: usize = 128;
pub struct Disk {
    pub sess: ClientSession,
    rgate: RecvGate,
    sgate: SendGate,
    //sel: Selector
}

enum Operation {
    Read,
    Write,
}

impl Operation {
    fn to_i32(&self) -> i32 {
        match self {
            Operation::Read => 0,
            Operation::Write => 1,
        }
    }
}

impl Disk {
    pub fn new(name: &str) -> Result<Self, Error> {
        let sels = crate::pes::VPE::cur().alloc_sels(2);
        let sess = ClientSession::new_with_sel(name, sels + 1)?;
        let mut rgate = RecvGate::new(math::next_log2(MSG_SIZE * 8), math::next_log2(MSG_SIZE))?;

        //Whats usually in obtain_sgate

        let crd = crate::kif::CapRngDesc::new(crate::kif::CapType::OBJECT, sels, 1);
        sess.obtain_for(
            crate::pes::VPE::cur().sel(),
            crd,
            |_slice_sink| {},
            |_slice_source| Ok(()),
        )?;
        //Connect sgate to disk session
        let sgate = SendGate::new_bind(sels);

        rgate
            .activate()
            .expect("failed to activate disk client session rgate!");

        Ok(Disk { sess, rgate, sgate })
    }

    pub fn rgate(&self) -> &RecvGate {
        &self.rgate
    }

    pub fn sgate(&self) -> &SendGate {
        &self.sgate
    }

    pub fn read(
        &self,
        cap: u32,
        bno: u32,
        len: usize,
        blocksize: usize,
        off: Option<goff>,
    ) -> Result<(), Error> {
        let off = if let Some(g) = off { g } else { 0 };
        if let Err(e) = send_recv_res!(
            &self.sgate,
            &self.rgate,
            Operation::Read.to_i32(),
            cap,
            bno,
            len,
            blocksize,
            off
        ) {
            Err(e)
        }
        else {
            Ok(())
        }
    }

    pub fn write(
        &self,
        cap: u32,
        bno: u32,
        len: usize,
        blocksize: usize,
        off: Option<goff>,
    ) -> Result<(), Error> {
        let off = if let Some(g) = off { g } else { 0 };

        if let Err(e) = send_recv_res!(
            &self.sgate,
            &self.rgate,
            Operation::Write.to_i32(),
            cap,
            bno,
            len,
            blocksize,
            off
        ) {
            Err(e)
        }
        else {
            Ok(())
        }
    }
}
