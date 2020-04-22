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

use base::cell::StaticCell;
use base::cfg;
use base::col::Vec;
use base::errors::{Code, Error};
use base::goff;
use base::kif;
use base::math;
use base::mem::GlobAddr;
use base::rc::Rc;
use base::tcu;

use arch::ktcu;
use cap::{Capability, KMemObject, KObject, MGateObject, PEObject};
use mem::{self, Allocation};
use pes::pemng;
use pes::{VPEFlags, VPEId, VPE};
use platform;

pub const MAX_VPES: usize = 64;

pub struct VPEMng {
    vpes: Vec<Option<Rc<VPE>>>,
    count: usize,
    next_id: usize,
}

static INST: StaticCell<Option<VPEMng>> = StaticCell::new(None);

pub fn get() -> &'static mut VPEMng {
    INST.get_mut().as_mut().unwrap()
}

pub fn init() {
    INST.set(Some(VPEMng {
        vpes: vec![None; MAX_VPES],
        count: 0,
        next_id: 0,
    }));
}

pub fn deinit() {
    INST.set(None);
}

impl VPEMng {
    pub fn count(&self) -> usize {
        self.count
    }

    pub fn vpe(&self, id: VPEId) -> Option<Rc<VPE>> {
        self.vpes[id].as_ref().map(|v| v.clone())
    }

    pub fn get_id(&mut self) -> Result<usize, Error> {
        for id in self.next_id..MAX_VPES {
            if self.vpes[id].is_none() {
                self.next_id = id + 1;
                return Ok(id);
            }
        }

        for id in 0..self.next_id {
            if self.vpes[id].is_none() {
                self.next_id = id + 1;
                return Ok(id);
            }
        }

        Err(Error::new(Code::NoSpace))
    }

    pub fn create(
        &mut self,
        name: &str,
        pe: Rc<PEObject>,
        eps_start: tcu::EpId,
        kmem: Rc<KMemObject>,
        flags: VPEFlags,
    ) -> Result<Rc<VPE>, Error> {
        let id: VPEId = self.get_id()?;
        let pe_id = pe.pe();

        let vpe: Rc<VPE> = VPE::new(name, id, pe, eps_start, kmem, flags);

        klog!(VPES, "Created VPE {} [id={}, pe={}]", name, id, pe_id);

        pemng::get().pemux(pe_id).add_vpe(id);
        if flags.is_empty() {
            pemng::get().init_vpe(&vpe).unwrap();
        }

        let res = vpe.clone();
        self.vpes[id] = Some(vpe);
        self.count += 1;
        Ok(res)
    }

    pub fn start_root(&mut self) -> Result<(), Error> {
        // TODO temporary
        let isa = platform::pe_desc(platform::kernel_pe()).isa();
        let pe_emem = kif::PEDesc::new(kif::PEType::COMP_EMEM, isa, 0);
        let pe_imem = kif::PEDesc::new(kif::PEType::COMP_IMEM, isa, 0);

        let pe_id = pemng::get()
            .find_pe(&pe_emem)
            .unwrap_or_else(|| pemng::get().find_pe(&pe_imem).unwrap());
        let pemux = pemng::get().pemux(pe_id);

        let kmem = KMemObject::new(mem::KERNEL_MEM - cfg::FIXED_KMEM);
        let vpe = self
            .create(
                "root",
                pemux.pe().clone(),
                tcu::FIRST_USER_EP,
                kmem,
                VPEFlags::BOOTMOD,
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

            vpe.obj_caps().borrow_mut().insert(cap);
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

            vpe.obj_caps().borrow_mut().insert(cap);
            sel += 1;
        }

        // PES
        for pe in platform::user_pes() {
            let pe_obj = pemng::get().pemux(pe).pe().clone();
            let cap = Capability::new(sel, KObject::PE(pe_obj));
            vpe.obj_caps().borrow_mut().insert(cap);
            sel += 1;
        }

        // memory
        for m in mem::get().mods() {
            if m.mem_type() != mem::MemType::KERNEL {
                let alloc = Allocation::new(m.addr(), m.capacity());
                let cap = Capability::new(
                    sel,
                    KObject::MGate(MGateObject::new(alloc, kif::Perm::RWX, false)),
                );

                vpe.obj_caps().borrow_mut().insert(cap);
                sel += 1;
            }
        }

        // let root know the first usable selector
        vpe.set_first_sel(sel);

        // go!
        pemng::get().init_vpe(&vpe)?;
        VPE::start_app(&vpe, 0)
    }

    pub fn remove(&mut self, id: VPEId) {
        // Replace item at position
        // https://stackoverflow.com/questions/33204273/how-can-i-take-ownership-of-a-vec-element-and-replace-it-with-something-else
        let vpe: Option<Rc<VPE>> = core::mem::replace(&mut self.vpes[id], None);

        match vpe {
            Some(ref v) => {
                let pemux = pemng::get().pemux(v.pe_id());
                pemux.rem_vpe(v.id());
                Self::destroy_vpe(v);
                self.count -= 1;
            },
            None => panic!("Removing nonexisting VPE with id {}", id)
        };
    }

    #[cfg(target_os = "linux")]
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

    fn destroy_vpe(vpe: &Rc<VPE>) {
        vpe.destroy();

        // assert!(Rc::strong_count(&vpe) == 1);

        // TODO temporary
        ktcu::reset_pe(vpe.pe_id()).unwrap();
        klog!(
            VPES,
            "Removed VPE {} [id={}, pe={}]",
            vpe.name(),
            vpe.id(),
            vpe.pe_id()
        );
    }
}

impl Drop for VPEMng {
    fn drop(&mut self) {
        for v in self.vpes.drain(0..) {
            if let Some(ref vpe) = v {
                #[cfg(target_os = "linux")]
                ::arch::childs::kill_child(vpe.pid());

                Self::destroy_vpe(vpe);
            }
        }
    }
}
