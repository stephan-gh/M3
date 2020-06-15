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
use base::errors::Error;
use base::goff;
use base::kif::{self, CapRngDesc, CapSel, CapType, PEDesc};
use base::rc::Rc;
use base::tcu::Label;
use base::tcu::{EpId, PEId, STD_EPS_COUNT, UPCALL_REP_OFF};
use base::util;
use core::fmt;
use thread;

use arch::loader::Loader;
use cap::{CapTable, Capability, KMemObject, KObject, PEObject};
use com::SendQueue;
use ktcu;
use pes::{pemng, vpemng};
use platform;

pub type VPEId = usize;

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
    pid: Cell<Option<i32>>,
    state: Cell<State>,
    name: String,
    flags: Cell<VPEFlags>,
    obj_caps: RefCell<CapTable>,
    map_caps: RefCell<CapTable>,
    pe: Rc<PEObject>,
    kmem: Rc<KMemObject>,
    rbuf_phys: Cell<goff>,
    eps_start: EpId,
    exit_code: Cell<Option<i32>>,
    upcalls: RefCell<SendQueue>,
    wait_sels: RefCell<Vec<u64>>,
    first_sel: Cell<CapSel>,
}

impl VPE {
    pub fn new(
        name: &str,
        id: VPEId,
        pe: Rc<PEObject>,
        eps_start: EpId,
        kmem: Rc<KMemObject>,
        flags: VPEFlags,
    ) -> Rc<Self> {
        let vpe = Rc::new(VPE {
            id,
            kmem,
            pid: Cell::from(None),
            state: Cell::from(State::INIT),
            name: name.to_string(),
            flags: Cell::from(flags),
            obj_caps: RefCell::from(CapTable::new()),
            map_caps: RefCell::from(CapTable::new()),
            rbuf_phys: Cell::from(0),
            eps_start,
            exit_code: Cell::from(None),
            upcalls: RefCell::from(SendQueue::new(id as u64, pe.pe())),
            pe,
            wait_sels: RefCell::from(Vec::new()),
            first_sel: Cell::from(kif::FIRST_FREE_SEL),
        });

        {
            vpe.obj_caps.borrow_mut().set_vpe(&vpe);
            vpe.map_caps.borrow_mut().set_vpe(&vpe);

            // kmem cap
            vpe.obj_caps().borrow_mut().insert(Capability::new(
                kif::SEL_KMEM,
                KObject::KMEM(vpe.kmem.clone()),
            ));
            // PE cap
            vpe.obj_caps()
                .borrow_mut()
                .insert(Capability::new(kif::SEL_PE, KObject::PE(vpe.pe.clone())));
            // cap for own VPE
            vpe.obj_caps()
                .borrow_mut()
                .insert(Capability::new(kif::SEL_VPE, KObject::VPE(Rc::downgrade(&vpe))));

            // alloc standard EPs
            let pemux = pemng::get().pemux(vpe.pe_id());
            pemux.alloc_eps(eps_start, STD_EPS_COUNT as u32);
            vpe.pe.alloc(STD_EPS_COUNT as u32);
        }

        vpe
    }

    pub fn init(vpe: &Rc<Self>) -> Result<(), Error> {
        let loader = Loader::get();
        loader.init_memory(vpe)?;

        if !platform::pe_desc(vpe.pe_id()).is_device() {
            vpe.init_eps()
        }
        else {
            Ok(())
        }
    }

    pub fn start(vpe: &Rc<Self>) -> Result<(), Error> {
        let loader = Loader::get();
        let pid = loader.start(&vpe)?;
        vpe.pid.set(Some(pid));
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn init_eps(&self) -> Result<(), Error> {
        Ok(())
    }

    #[cfg(target_os = "none")]
    fn init_eps(&self) -> Result<(), Error> {
        use base::cfg;
        use base::kif::Perm;
        use base::tcu;
        use cap::{RGateObject, SGateObject};

        let pemux = pemng::get().pemux(self.pe_id());
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
                    .translate(self.id(), rbuf_virt as goff, Perm::RW)?
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

    pub fn pe(&self) -> &Rc<PEObject> {
        &self.pe
    }

    pub fn pe_id(&self) -> PEId {
        self.pe.pe()
    }

    pub fn pe_desc(&self) -> PEDesc {
        platform::pe_desc(self.pe_id())
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
        self.flags.get().contains(VPEFlags::IS_ROOT)
    }

    pub fn set_mem_base(&self, addr: goff) {
        pemng::get().pemux(self.pe_id()).set_mem_base(addr);
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

    pub fn wait() {
        let event = &EXIT_EVENT as *const _ as thread::Event;
        thread::ThreadManager::get().wait_for(event);
    }

    fn check_exits(vpe: &Rc<Self>, reply: &mut kif::syscalls::VPEWaitReply) -> bool {
        {
            for sel in &*vpe.wait_sels.borrow() {
                let wvpe = vpe
                    .obj_caps()
                    .borrow()
                    .get(*sel as CapSel)
                    .map(|c| c.get().clone());
                match wvpe {
                    Some(KObject::VPE(wv)) => {
                        let wv = wv.upgrade().unwrap();
                        if wv.id() == vpe.id() {
                            continue;
                        }

                        if let Some(code) = wv.fetch_exit_code() {
                            reply.vpe_sel = *sel as u64;
                            reply.exitcode = code as u64;
                            return true;
                        }
                    },
                    _ => continue,
                }
            }
        }

        Self::wait();
        false
    }

    pub fn wait_exit_async(
        vpe: &Rc<Self>,
        sels: &[u64],
        reply: &mut kif::syscalls::VPEWaitReply,
    ) -> bool {
        let was_empty = vpe.wait_sels.borrow().len() == 0;

        vpe.wait_sels.borrow_mut().clear();
        vpe.wait_sels.borrow_mut().extend_from_slice(sels);

        if !was_empty {
            return false;
        }

        loop {
            if Self::check_exits(vpe, reply) {
                break;
            }
        }

        vpe.wait_sels.borrow_mut().clear();
        true
    }

    pub fn upcall_vpewait(&self, event: u64, reply: &kif::syscalls::VPEWaitReply) {
        let msg = kif::upcalls::VPEWait {
            def: kif::upcalls::DefaultUpcall {
                opcode: kif::upcalls::Operation::VPEWAIT.val,
                event,
            },
            error: reply.error,
            vpe_sel: reply.vpe_sel,
            exitcode: reply.exitcode,
        };

        klog!(
            UPCALLS,
            "Sending upcall VPEWAIT (error={}, event={}, vpe={}, exitcode={}) to VPE {}",
            { msg.error },
            { msg.def.event },
            { msg.vpe_sel },
            { msg.exitcode },
            self.id()
        );
        self.upcalls
            .borrow_mut()
            .send(
                self.eps_start + UPCALL_REP_OFF,
                0,
                util::object_to_bytes(&msg),
            )
            .unwrap();
    }

    pub fn start_app(vpe: &Rc<Self>, pid: Option<i32>) -> Result<(), Error> {
        if vpe.state.get() != State::INIT {
            return Ok(());
        }

        vpe.pid.set(pid);
        vpe.state.set(State::RUNNING);

        vpemng::get().start_vpe(&vpe)
    }

    pub fn stop_app(vpe: &Rc<Self>, exit_code: i32, is_self: bool) {
        if vpe.state.get() == State::DEAD {
            return;
        }

        klog!(VPES, "Stopping VPE {} [id={}]", vpe.name(), vpe.id());

        if is_self {
            Self::exit_app(vpe, exit_code, false);
        }
        else {
            if vpe.state.get() == State::RUNNING {
                // devices always exit successfully
                let exit_code = if vpe.pe_desc().is_device() { 0 } else { 1 };
                Self::exit_app(vpe, exit_code, true);
            }
            else {
                vpe.state.set(State::DEAD);
                vpemng::get().stop_vpe(&vpe, false, true).unwrap();
                ktcu::drop_msgs(ktcu::KSYS_EP, vpe.id() as Label);
            }
        }
    }

    fn exit_app(vpe: &Rc<Self>, exit_code: i32, stop: bool) {
        #[cfg(target_os = "linux")]
        if let Some(pid) = vpe.pid() {
            // first kill the process to ensure that it cannot use EPs anymore
            ktcu::reset_pe(vpe.pe_id(), pid).unwrap();
        }

        // TODO force-invalidate all EPs of this VPE

        #[cfg(target_os = "none")]
        {
            let pemux = pemng::get().pemux(vpe.pe_id());
            // TODO force-invalidate *all* EPs of this VPE
            for ep in vpe.eps_start..vpe.eps_start + STD_EPS_COUNT {
                // ignore failures
                pemux.invalidate_ep(vpe.id(), ep, true, false).ok();
            }
        }

        // make sure that we don't get further syscalls by this VPE
        ktcu::drop_msgs(ktcu::KSYS_EP, vpe.id() as Label);

        vpe.state.set(State::DEAD);
        vpe.exit_code.set(Some(exit_code));

        vpemng::get().stop_vpe(&vpe, stop, false).unwrap();

        vpe.revoke_caps(false);

        let event = &EXIT_EVENT as *const _ as thread::Event;
        thread::ThreadManager::get().notify(event, None);

        // if it's root, there is nobody waiting for it; just remove it
        if vpe.is_root() {
            vpemng::get().remove_vpe(vpe.id());
        }
    }

    fn revoke_caps(&self, all: bool) {
        self.obj_caps.borrow_mut().revoke_all(all);
        self.map_caps.borrow_mut().revoke_all(true);
    }

    pub fn revoke(vpe: &Rc<Self>, crd: CapRngDesc, own: bool) {
        // we can't use borrow_mut() here, because revoke might need to use borrow as well.
        if crd.cap_type() == CapType::OBJECT {
            vpe.obj_caps().borrow_mut().revoke(crd, own);
        }
        else {
            vpe.map_caps().borrow_mut().revoke(crd, own);
        }
    }
}

impl Drop for VPE {
    fn drop(&mut self) {
        self.state.set(State::DEAD);

        // free standard EPs
        let pemux = pemng::get().pemux(self.pe_id());
        pemux.free_eps(self.eps_start, STD_EPS_COUNT as u32);
        self.pe.free(STD_EPS_COUNT as u32);

        self.revoke_caps(true);

        // TODO temporary
        if let Some(pid) = self.pid() {
            ktcu::reset_pe(self.pe_id(), pid).unwrap();
        }

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
