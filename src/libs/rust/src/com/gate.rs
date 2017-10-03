use core::ops;
use cap;
use cap::Capability;
use com::EpMux;
use dtu;
use errors::Error;

pub type EpId = dtu::EpId;

pub const INVALID_EP: EpId = dtu::EP_COUNT;

pub struct Gate {
    pub cap: Capability,
    pub ep: EpId,
}

impl Gate {
    pub fn new(sel: cap::Selector, flags: cap::Flags) -> Gate {
        Gate {
            cap: Capability::new(sel, flags),
            ep: INVALID_EP,
        }
    }

    pub fn activate(&mut self) -> Result<(), Error> {
        if self.ep == INVALID_EP {
            try!(EpMux::get().switch_to(self));
        }
        Ok(())
    }
}

impl ops::Drop for Gate {
    fn drop(&mut self) {
        if self.ep != INVALID_EP {
            EpMux::get().remove(self);
        }
    }
}
