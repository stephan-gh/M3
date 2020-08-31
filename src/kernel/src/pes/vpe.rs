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

use base::cell::{Cell, RefCell};
use base::col::{String, ToString, Vec};
use base::errors::{Code, Error};
use base::goff;
use base::kif::{self, CapRngDesc, CapSel, CapType, PEDesc};
use base::rc::{Rc, SRc};
use base::tcu::Label;
use base::tcu::{EpId, PEId, VPEId, STD_EPS_COUNT, UPCALL_REP_OFF};
use base::util;
use core::fmt;
use thread;

use crate::arch::loader::Loader;
use crate::cap::{CapTable, Capability, EPObject, KMemObject, KObject, PEObject};
use crate::com::SendQueue;
use crate::ktcu;
use crate::pes::{PEMng, VPEMng};
use crate::platform;

bitflags! {
    pub struct VPEFlags : u32 {
        const IS_ROOT     = 1;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum State {
    INIT,
    RUNNING,
    DEAD,
}

pub const KERNEL_ID: VPEId = 0xFFFF;
pub const INVAL_ID: VPEId = 0xFFFF;

static EXIT_EVENT: i32 = 0;

pub struct VPE {
    id: VPEId,
    name: String,
    flags: VPEFlags,
    eps_start: EpId,

    pe: SRc<PEObject>,
    kmem: SRc<KMemObject>,

    state: Cell<State>,
    pid: Cell<Option<i32>>,
    exit_code: Cell<Option<i32>>,
    first_sel: Cell<CapSel>,

    obj_caps: RefCell<CapTable>,
    map_caps: RefCell<CapTable>,

    eps: RefCell<Vec<Rc<EPObject>>>,
    rbuf_phys: Cell<goff>,
    upcalls: RefCell<SendQueue>,
    wait_sels: RefCell<Vec<u64>>,
}

impl VPE {
    pub fn new(
        name: &str,
        id: VPEId,
        pe: SRc<PEObject>,
        eps_start: EpId,
        kmem: SRc<KMemObject>,
        flags: VPEFlags,
    ) -> Result<Rc<Self>, Error> {
        let vpe = Rc::new(VPE {
            id,
            name: name.to_string(),
            flags,
            eps_start,
            kmem,
            state: Cell::from(State::INIT),
            pid: Cell::from(None),
            exit_code: Cell::from(None),
            first_sel: Cell::from(kif::FIRST_FREE_SEL),
            obj_caps: RefCell::from(CapTable::default()),
            map_caps: RefCell::from(CapTable::default()),
            eps: RefCell::from(Vec::new()),
            rbuf_phys: Cell::from(0),
            upcalls: RefCell::from(SendQueue::new(id as u64, pe.pe())),
            wait_sels: RefCell::from(Vec::new()),
            pe,
        });

        {
            vpe.obj_caps.borrow_mut().set_vpe(&vpe);
            vpe.map_caps.borrow_mut().set_vpe(&vpe);

            // kmem cap
            vpe.obj_caps().borrow_mut().insert(Capability::new(
                kif::SEL_KMEM,
                KObject::KMem(vpe.kmem.clone()),
            ))?;
            // PE cap
            vpe.obj_caps()
                .borrow_mut()
                .insert(Capability::new(kif::SEL_PE, KObject::PE(vpe.pe.clone())))?;
            // cap for own VPE
            vpe.obj_caps().borrow_mut().insert(Capability::new(
                kif::SEL_VPE,
                KObject::VPE(Rc::downgrade(&vpe)),
            ))?;

            // alloc standard EPs
            let pemux = PEMng::get().pemux(vpe.pe_id());
            pemux.alloc_eps(eps_start, STD_EPS_COUNT as u32);
            vpe.pe.alloc(STD_EPS_COUNT as u32);

            // add us to PE
            vpe.pe.add_vpe();
        }

        Ok(vpe)
    }

    pub fn init_async(&self) -> Result<(), Error> {
        #[cfg(target_os = "none")]
        {
            let loader = Loader::get();
            loader.init_memory_async(self)?;
            if !platform::pe_desc(self.pe_id()).is_device() {
                self.init_eps_async()
            }
            else {
                Ok(())
            }
        }

        #[cfg(target_os = "linux")]
        Ok(())
    }

    #[cfg(target_os = "none")]
    fn init_eps_async(&self) -> Result<(), Error> {
        use crate::cap::{RGateObject, SGateObject};
        use base::cfg;
        use base::kif::Perm;
        use base::tcu;

        let pemux = PEMng::get().pemux(self.pe_id());
        let vpe = if platform::is_shared(self.pe_id()) {
            self.id()
        }
        else {
            INVAL_ID
        };

        // get physical address of receive buffer
        let rbuf_virt = platform::pe_desc(self.pe_id()).rbuf_std_space().0;
        self.rbuf_phys
            .set(if platform::pe_desc(self.pe_id()).has_virtmem() {
                pemux
                    .translate_async(self.id(), rbuf_virt as goff, Perm::RW)?
                    .raw()
            }
            else {
                rbuf_virt as goff
            });

        // attach syscall send endpoint
        {
            let rgate = RGateObject::new(cfg::SYSC_RBUF_ORD, cfg::SYSC_RBUF_ORD);
            rgate.activate(platform::kernel_pe(), ktcu::KSYS_EP, 0xDEADBEEF);
            let sgate = SGateObject::new(&rgate, self.id() as tcu::Label, 1);
            pemux.config_snd_ep(self.eps_start + tcu::SYSC_SEP_OFF, vpe, &sgate)?;
        }

        // attach syscall receive endpoint
        let mut rbuf_addr = self.rbuf_phys.get();
        {
            let rgate = RGateObject::new(cfg::SYSC_RBUF_ORD, cfg::SYSC_RBUF_ORD);
            rgate.activate(self.pe_id(), self.eps_start + tcu::SYSC_REP_OFF, rbuf_addr);
            pemux.config_rcv_ep(self.eps_start + tcu::SYSC_REP_OFF, vpe, None, &rgate)?;
            rbuf_addr += cfg::SYSC_RBUF_SIZE as goff;
        }

        // attach upcall receive endpoint
        {
            let rgate = RGateObject::new(cfg::UPCALL_RBUF_ORD, cfg::UPCALL_RBUF_ORD);
            rgate.activate(
                self.pe_id(),
                self.eps_start + tcu::UPCALL_REP_OFF,
                rbuf_addr,
            );
            pemux.config_rcv_ep(
                self.eps_start + tcu::UPCALL_REP_OFF,
                vpe,
                Some(self.eps_start + tcu::UPCALL_RPLEP_OFF),
                &rgate,
            )?;
            rbuf_addr += cfg::UPCALL_RBUF_SIZE as goff;
        }

        // attach default receive endpoint
        {
            let rgate = RGateObject::new(cfg::DEF_RBUF_ORD, cfg::DEF_RBUF_ORD);
            rgate.activate(self.pe_id(), self.eps_start + tcu::DEF_REP_OFF, rbuf_addr);
            pemux.config_rcv_ep(self.eps_start + tcu::DEF_REP_OFF, vpe, None, &rgate)?;
        }

        Ok(())
    }

    pub fn id(&self) -> VPEId {
        self.id
    }

    pub fn pe(&self) -> &SRc<PEObject> {
        &self.pe
    }

    pub fn pe_id(&self) -> PEId {
        self.pe.pe()
    }

    pub fn pe_desc(&self) -> PEDesc {
        platform::pe_desc(self.pe_id())
    }

    pub fn kmem(&self) -> &SRc<KMemObject> {
        &self.kmem
    }

    pub fn rbuf_addr(&self) -> goff {
        self.rbuf_phys.get()
    }

    pub fn eps_start(&self) -> EpId {
        self.eps_start
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn obj_caps(&self) -> &RefCell<CapTable> {
        &self.obj_caps
    }

    pub fn map_caps(&self) -> &RefCell<CapTable> {
        &self.map_caps
    }

    pub fn state(&self) -> State {
        self.state.get()
    }

    pub fn is_root(&self) -> bool {
        self.flags.contains(VPEFlags::IS_ROOT)
    }

    pub fn set_mem_base(&self, addr: goff) {
        PEMng::get().pemux(self.pe_id()).set_mem_base(addr);
    }

    pub fn first_sel(&self) -> CapSel {
        self.first_sel.get()
    }

    pub fn set_first_sel(&self, sel: CapSel) {
        self.first_sel.set(sel);
    }

    pub fn pid(&self) -> Option<i32> {
        self.pid.get()
    }

    pub fn fetch_exit_code(&self) -> Option<i32> {
        self.exit_code.replace(None)
    }

    pub fn add_ep(&self, ep: Rc<EPObject>) {
        self.eps.borrow_mut().push(ep);
    }

    pub fn rem_ep(&self, ep: &Rc<EPObject>) {
        self.eps.borrow_mut().retain(|e| e.ep() != ep.ep());
    }

    pub fn wait_async() {
        let event = &EXIT_EVENT as *const _ as thread::Event;
        thread::ThreadManager::get().wait_for(event);
    }

    pub fn start_wait(&self, sels: &[u64]) -> bool {
        let was_empty = self.wait_sels.borrow().len() == 0;

        self.wait_sels.borrow_mut().clear();
        self.wait_sels.borrow_mut().extend_from_slice(sels);

        was_empty
    }

    fn fetch_exit(&self) -> Option<(CapSel, i32)> {
        for sel in &*self.wait_sels.borrow() {
            let wvpe = self
                .obj_caps()
                .borrow()
                .get(*sel as CapSel)
                .map(|c| c.get().clone());
            match wvpe {
                Some(KObject::VPE(wv)) => {
                    let wv = wv.upgrade().unwrap();
                    if wv.id() == self.id() {
                        continue;
                    }

                    if let Some(code) = wv.fetch_exit_code() {
                        return Some((*sel, code));
                    }
                },
                _ => continue,
            }
        }

        None
    }

    pub fn wait_exit_async(&self) -> Option<(CapSel, i32)> {
        assert!(self.wait_sels.borrow().len() > 0);

        let res = loop {
            if let Some(res) = self.fetch_exit() {
                break Some(res);
            }

            if self.state() != State::RUNNING {
                break None;
            }

            Self::wait_async();
        };

        self.wait_sels.borrow_mut().clear();
        res
    }

    pub fn upcall_vpe_wait(&self, event: u64, vpe_sel: CapSel, exitcode: i32) {
        let msg = kif::upcalls::VPEWait {
            def: kif::upcalls::DefaultUpcall {
                opcode: kif::upcalls::Operation::VPE_WAIT.val,
                event,
            },
            error: Code::None as u64,
            vpe_sel: vpe_sel as u64,
            exitcode: exitcode as u64,
        };

        self.send_upcall(&msg);
    }

    pub fn upcall_derive_srv(&self, event: u64, result: Result<(), Error>) {
        let msg = kif::upcalls::DeriveSrv {
            def: kif::upcalls::DefaultUpcall {
                opcode: kif::upcalls::Operation::DERIVE_SRV.val,
                event,
            },
            error: Code::from(result) as u64,
        };

        self.send_upcall(&msg);
    }

    fn send_upcall<M: fmt::Debug>(&self, msg: &M) {
        klog!(UPCALLS, "Sending upcall {:?} to VPE {}", msg, self.id());

        self.upcalls
            .borrow_mut()
            .send(
                self.eps_start + UPCALL_REP_OFF,
                0,
                util::object_to_bytes(msg),
            )
            .unwrap();
    }

    pub fn start_app_async(&self, pid: Option<i32>) -> Result<(), Error> {
        if self.state.get() != State::INIT {
            return Ok(());
        }

        self.pid.set(pid);
        self.state.set(State::RUNNING);

        VPEMng::get().start_vpe_async(self)?;

        let loader = Loader::get();
        let pid = loader.start(self)?;
        self.pid.set(Some(pid));

        Ok(())
    }

    pub fn stop_app_async(&self, exit_code: i32, is_self: bool) {
        if self.state.get() == State::DEAD {
            return;
        }

        klog!(VPES, "Stopping VPE {} [id={}]", self.name(), self.id());

        if is_self {
            self.exit_app_async(exit_code, false);
        }
        else if self.state.get() == State::RUNNING {
            // devices always exit successfully
            let exit_code = if self.pe_desc().is_device() { 0 } else { 1 };
            self.exit_app_async(exit_code, true);
        }
        else {
            self.state.set(State::DEAD);
            VPEMng::get().stop_vpe_async(self, true, true).unwrap();
            ktcu::drop_msgs(ktcu::KSYS_EP, self.id() as Label);
        }
    }

    fn exit_app_async(&self, exit_code: i32, stop: bool) {
        #[cfg(target_os = "linux")]
        if let Some(pid) = self.pid() {
            // first kill the process to ensure that it cannot use EPs anymore
            ktcu::reset_pe(self.pe_id(), pid).unwrap();
        }

        #[cfg(target_os = "none")]
        {
            let pemux = PEMng::get().pemux(self.pe_id());
            // force-invalidate standard EPs
            for ep in self.eps_start..self.eps_start + STD_EPS_COUNT as EpId {
                // ignore failures
                pemux.invalidate_ep(self.id(), ep, true, false).ok();
            }

            // force-invalidate all other EPs of this VPE
            for ep in &*self.eps.borrow_mut() {
                // ignore failures here
                ep.deconfigure(true).ok();
            }
        }

        // make sure that we don't get further syscalls by this VPE
        ktcu::drop_msgs(ktcu::KSYS_EP, self.id() as Label);

        self.state.set(State::DEAD);
        self.exit_code.set(Some(exit_code));

        self.force_stop_async(stop);

        let event = &EXIT_EVENT as *const _ as thread::Event;
        thread::ThreadManager::get().notify(event, None);

        // if it's root, there is nobody waiting for it; just remove it
        if self.is_root() {
            VPEMng::get().remove_vpe_async(self.id());
        }
    }

    fn revoke_caps_async(&self) {
        self.obj_caps.borrow_mut().revoke_all_async();
        self.map_caps.borrow_mut().revoke_all_async();
    }

    pub fn revoke_async(&self, crd: CapRngDesc, own: bool) -> Result<(), Error> {
        // we can't use borrow_mut() here, because revoke might need to use borrow as well.
        if crd.cap_type() == CapType::OBJECT {
            self.obj_caps().borrow_mut().revoke_async(crd, own)
        }
        else {
            self.map_caps().borrow_mut().revoke_async(crd, own)
        }
    }

    pub fn force_stop_async(&self, stop: bool) {
        VPEMng::get().stop_vpe_async(self, stop, true).unwrap();

        self.revoke_caps_async();
    }
}

impl Drop for VPE {
    fn drop(&mut self) {
        self.state.set(State::DEAD);

        // free standard EPs
        let pemux = PEMng::get().pemux(self.pe_id());
        pemux.free_eps(self.eps_start, STD_EPS_COUNT as u32);
        self.pe.free(STD_EPS_COUNT as u32);

        // remove us from PE
        self.pe.rem_vpe();

        assert!(self.obj_caps.borrow().is_empty());
        assert!(self.map_caps.borrow().is_empty());

        klog!(
            VPES,
            "Removed VPE {} [id={}, pe={}]",
            self.name(),
            self.id(),
            self.pe_id()
        );
    }
}

impl fmt::Debug for VPE {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "VPE[id={}, pe={}, name={}, state={:?}]",
            self.id(),
            self.pe_id(),
            self.name(),
            self.state()
        )
    }
}
