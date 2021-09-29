/*
 * Copyright (C) 2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
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

use base::cell::StaticUnsafeCell;
use base::cfg;
use base::col::Vec;
use base::errors::{Code, Error};
use base::goff;
use base::kif;
use base::math;
use base::mem::GlobAddr;
use base::rc::{Rc, SRc};
use base::tcu;
use base::vec;

use crate::args;
use crate::cap::{Capability, KMemObject, KObject, MGateObject, PEObject, RGateObject};
use crate::ktcu;
use crate::mem::{self, Allocation};
use crate::pes::{PEMng, State, VPEFlags, VPE};
use crate::platform;

pub struct VPEMng {
    vpes: Vec<Option<Rc<VPE>>>,
    count: usize,
    next_id: tcu::VPEId,
}

// TODO can we use a safe cell here?
static INST: StaticUnsafeCell<Option<VPEMng>> = StaticUnsafeCell::new(None);

pub fn init() {
    INST.set(Some(VPEMng {
        vpes: vec![None; cfg::MAX_VPES],
        count: 0,
        next_id: 0,
    }));
}

pub fn deinit() {
    INST.set(None);
}

impl VPEMng {
    pub fn get() -> &'static mut Self {
        INST.get_mut().as_mut().unwrap()
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn vpe(&self, id: tcu::VPEId) -> Option<Rc<VPE>> {
        self.vpes[id as usize].as_ref().cloned()
    }

    fn get_id(&mut self) -> Result<tcu::VPEId, Error> {
        for id in self.next_id..cfg::MAX_VPES as tcu::VPEId {
            if self.vpes[id as usize].is_none() {
                self.next_id = id + 1;
                return Ok(id);
            }
        }

        for id in 0..self.next_id {
            if self.vpes[id as usize].is_none() {
                self.next_id = id + 1;
                return Ok(id);
            }
        }

        Err(Error::new(Code::NoSpace))
    }

    pub fn create_vpe_async(
        &mut self,
        name: &str,
        pe: SRc<PEObject>,
        eps_start: tcu::EpId,
        kmem: SRc<KMemObject>,
        flags: VPEFlags,
    ) -> Result<Rc<VPE>, Error> {
        let id: tcu::VPEId = self.get_id()?;
        let pe_id = pe.pe();

        let vpe = VPE::new(name, id, pe, eps_start, kmem, flags)?;

        klog!(VPES, "Created VPE {} [id={}, pe={}]", name, id, pe_id);

        let clone = vpe.clone();
        self.vpes[id as usize] = Some(vpe);
        self.count += 1;

        PEMng::get().pemux(pe_id).add_vpe(id);
        if flags.is_empty() {
            self.init_vpe_async(&clone).unwrap();
        }

        Ok(clone)
    }

    fn init_vpe_async(&mut self, vpe: &VPE) -> Result<(), Error> {
        if platform::pe_desc(vpe.pe_id()).supports_pemux() {
            PEMng::get().pemux(vpe.pe_id()).vpe_ctrl_async(
                vpe.id(),
                vpe.eps_start(),
                kif::pemux::VPEOp::INIT,
            )?;
        }

        vpe.init_async()
    }

    pub fn start_vpe_async(&mut self, vpe: &VPE) -> Result<(), Error> {
        if platform::pe_desc(vpe.pe_id()).supports_pemux() {
            PEMng::get().pemux(vpe.pe_id()).vpe_ctrl_async(
                vpe.id(),
                vpe.eps_start(),
                kif::pemux::VPEOp::START,
            )
        }
        else {
            Ok(())
        }
    }

    pub fn stop_vpe_async(&mut self, vpe: &VPE, stop: bool, reset: bool) -> Result<(), Error> {
        if stop && platform::pe_desc(vpe.pe_id()).supports_pemux() {
            PEMng::get().pemux(vpe.pe_id()).vpe_ctrl_async(
                vpe.id(),
                vpe.eps_start(),
                kif::pemux::VPEOp::STOP,
            )?;
        }

        if reset && !platform::pe_desc(vpe.pe_id()).is_programmable() {
            ktcu::reset_pe(vpe.pe_id(), vpe.pid().unwrap_or(0))
        }
        else {
            Ok(())
        }
    }

    pub fn start_root_async(&mut self) -> Result<(), Error> {
        // TODO temporary
        let isa = platform::pe_desc(platform::kernel_pe()).isa();
        let pe_emem = kif::PEDesc::new(kif::PEType::COMP_EMEM, isa, 0);
        let pe_imem = kif::PEDesc::new(kif::PEType::COMP_IMEM, isa, 0);

        let pe_id = PEMng::get()
            .find_pe(&pe_emem)
            .unwrap_or_else(|| PEMng::get().find_pe(&pe_imem).unwrap());
        let pemux = PEMng::get().pemux(pe_id);

        let kmem = KMemObject::new(args::get().kmem - cfg::FIXED_KMEM);
        let vpe = self
            .create_vpe_async(
                "root",
                pemux.pe().clone(),
                tcu::FIRST_USER_EP,
                kmem,
                VPEFlags::IS_ROOT,
            )
            .expect("Unable to create VPE for root");

        let mut sel = kif::FIRST_FREE_SEL;

        // boot info
        {
            let alloc = Allocation::new(platform::info_addr(), platform::info_size() as goff);
            let cap = Capability::new(
                sel,
                KObject::MGate(MGateObject::new(alloc, kif::Perm::RWX, false)),
            );

            vpe.obj_caps().borrow_mut().insert(cap).unwrap();
            sel += 1;
        }

        // serial rgate
        {
            let cap = Capability::new(
                sel,
                KObject::RGate(RGateObject::new(
                    cfg::SERIAL_BUF_ORD,
                    cfg::SERIAL_BUF_ORD,
                    true,
                )),
            );
            vpe.obj_caps().borrow_mut().insert(cap).unwrap();
            sel += 1;
        }

        // boot modules
        for m in platform::mods() {
            let size = math::round_up(m.size as usize, cfg::PAGE_SIZE);
            let alloc = Allocation::new(GlobAddr::new(m.addr), size as goff);
            let cap = Capability::new(
                sel,
                KObject::MGate(MGateObject::new(alloc, kif::Perm::RWX, false)),
            );

            vpe.obj_caps().borrow_mut().insert(cap).unwrap();
            sel += 1;
        }

        // PES
        for pe in platform::user_pes() {
            let pe_obj = PEMng::get().pemux(pe).pe().clone();
            let cap = Capability::new(sel, KObject::PE(pe_obj));
            vpe.obj_caps().borrow_mut().insert(cap).unwrap();
            sel += 1;
        }

        // memory
        #[cfg(not(target_vendor = "host"))]
        let mut mem_ep = 1;

        for m in mem::borrow_mut().mods() {
            if m.mem_type() != mem::MemType::KERNEL {
                let alloc = Allocation::new(m.addr(), m.capacity());
                // create a derive MGateObject to prevent freeing the memory if it's of type ROOT
                let mgate_obj = MGateObject::new(alloc, kif::Perm::RWX, true);

                #[cfg(not(target_vendor = "host"))]
                {
                    // we currently assume that we have enough protection EPs for all user memory regions
                    assert!(mem_ep < tcu::PMEM_PROT_EPS as tcu::EpId);

                    // configure physical memory protection EP
                    pemux
                        .config_mem_ep(
                            mem_ep,
                            kif::pemux::VPE_ID as tcu::VPEId,
                            &mgate_obj,
                            m.addr().pe(),
                        )
                        .unwrap();
                    mem_ep += 1;
                }

                if m.mem_type() != mem::MemType::ROOT {
                    // insert capability
                    let cap = Capability::new(sel, KObject::MGate(mgate_obj));
                    vpe.obj_caps().borrow_mut().insert(cap).unwrap();
                    sel += 1;
                }
            }
        }

        // let root know the first usable selector
        vpe.set_first_sel(sel);

        // go!
        self.init_vpe_async(&vpe)?;
        vpe.start_app_async(None)
    }

    pub fn remove_vpe_async(&mut self, id: tcu::VPEId) {
        // Replace item at position
        // https://stackoverflow.com/questions/33204273/how-can-i-take-ownership-of-a-vec-element-and-replace-it-with-something-else
        let vpe: Option<Rc<VPE>> = base::mem::replace(&mut self.vpes[id as usize], None);

        match vpe {
            Some(ref v) => {
                let pemux = PEMng::get().pemux(v.pe_id());
                pemux.rem_vpe(v.id());
                v.force_stop_async(v.state() != State::DEAD);
                self.count -= 1;
            },
            None => panic!("Removing nonexisting VPE with id {}", id),
        };
    }

    #[cfg(target_vendor = "host")]
    pub fn find_vpe<P>(&self, pred: P) -> Option<Rc<VPE>>
    where
        P: Fn(&Rc<VPE>) -> bool,
    {
        for v in &self.vpes {
            if let Some(vpe) = v.as_ref() {
                if pred(&vpe) {
                    return Some(vpe.clone());
                }
            }
        }
        None
    }
}

impl Drop for VPEMng {
    fn drop(&mut self) {
        for _vpe in self.vpes.drain(0..).flatten() {
            #[cfg(target_vendor = "host")]
            if let Some(pid) = _vpe.pid() {
                crate::arch::childs::kill_child(pid);
            }
        }
    }
}
