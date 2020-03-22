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

use base::cell::RefCell;
use base::col::{String, ToString, Vec};
use base::tcu::{EpId, PEId, STD_EPS_COUNT, UPCALL_REP_OFF};
use base::errors::Error;
use base::goff;
use base::kif::{self, CapRngDesc, CapSel, CapType, PEDesc};
use base::rc::Rc;
use base::tcu::Label;
use base::util;
use core::fmt;
use core::mem;
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
        const BOOTMOD     = 0b00000001;
        const IDLE        = 0b00000010;
        const INIT        = 0b00000100;
        const HASAPP      = 0b00001000;
        const READY       = 0b00010000;
        const WAITING     = 0b00100000;
        const STOPPED     = 0b01000000;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum State {
    RUNNING,
    DEAD,
}

pub const KERNEL_ID: VPEId = 0xFFFF;
pub const INVAL_ID: VPEId = 0xFFFF;

static EXIT_EVENT: i32 = 0;

pub struct VPE {
    id: VPEId,
    pid: i32,
    state: State,
    name: String,
    flags: VPEFlags,
    obj_caps: CapTable,
    map_caps: CapTable,
    pe: Rc<RefCell<PEObject>>,
    kmem: Rc<RefCell<KMemObject>>,
    rbuf_phys: goff,
    eps_start: EpId,
    exit_code: Option<i32>,
    upcalls: SendQueue,
    wait_sels: Vec<u64>,
    first_sel: CapSel,
}

impl VPE {
    pub fn new(
        name: &str,
        id: VPEId,
        pe: Rc<RefCell<PEObject>>,
        eps_start: EpId,
        kmem: Rc<RefCell<KMemObject>>,
        flags: VPEFlags,
    ) -> Rc<RefCell<Self>> {
        let vpe = Rc::new(RefCell::new(VPE {
            id,
            pe: pe.clone(),
            kmem,
            pid: 0,
            state: State::DEAD,
            name: name.to_string(),
            flags,
            obj_caps: CapTable::new(),
            map_caps: CapTable::new(),
            rbuf_phys: 0,
            eps_start,
            exit_code: None,
            upcalls: SendQueue::new(id as u64, pe.borrow().pe()),
            wait_sels: Vec::new(),
            first_sel: kif::FIRST_FREE_SEL,
        }));

        {
            let mut vpe_mut = vpe.borrow_mut();
            unsafe {
                vpe_mut.obj_caps.set_vpe(vpe.as_ptr());
                vpe_mut.map_caps.set_vpe(vpe.as_ptr());
            }

            // kmem cap
            let kmem = vpe_mut.kmem.clone();
            vpe_mut
                .obj_caps_mut()
                .insert(Capability::new(kif::SEL_KMEM, KObject::KMEM(kmem)));
            // PE cap
            let pe = vpe_mut.pe.clone();
            vpe_mut
                .obj_caps_mut()
                .insert(Capability::new(kif::SEL_PE, KObject::PE(pe)));
            // cap for own VPE
            vpe_mut
                .obj_caps_mut()
                .insert(Capability::new(kif::SEL_VPE, KObject::VPE(vpe.clone())));

            // alloc standard EPs
            let pemux = pemng::get().pemux(vpe_mut.pe_id());
            pemux.alloc_eps(eps_start, STD_EPS_COUNT as u32);
            vpe_mut.pe.borrow_mut().alloc(STD_EPS_COUNT as u32);
        }

        vpe
    }

    pub fn init(vpe: &Rc<RefCell<VPE>>) -> Result<(), Error> {
        let mut vpe_mut = vpe.borrow_mut();

        let loader = Loader::get();
        loader.init_memory(&mut vpe_mut)?;
        vpe_mut.flags |= VPEFlags::HASAPP;

        if !platform::pe_desc(vpe_mut.pe_id()).is_device() {
            vpe_mut.init_eps()
        }
        else {
            Ok(())
        }
    }

    pub fn start(vpe: &Rc<RefCell<VPE>>) -> Result<(), Error> {
        let mut vpe_mut = vpe.borrow_mut();

        let loader = Loader::get();
        let pid = loader.start(&mut vpe_mut)?;
        vpe_mut.set_pid(pid);
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn init_eps(&mut self) -> Result<(), Error> {
        Ok(())
    }

    #[cfg(target_os = "none")]
    fn init_eps(&mut self) -> Result<(), Error> {
        use base::cfg;
        use base::tcu;
        use base::kif::Perm;
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
        self.rbuf_phys = if platform::pe_desc(self.pe_id()).has_virtmem() {
            pemux
                .translate(self.id(), rbuf_virt as goff, Perm::RW)?
                .raw()
        }
        else {
            rbuf_virt as goff
        };

        // attach syscall send endpoint
        {
            let rgate = RGateObject::new(cfg::SYSC_RBUF_ORD, cfg::SYSC_RBUF_ORD);
            {
                let mut rgate = rgate.borrow_mut();
                rgate.activate(platform::kernel_pe(), ktcu::KSYS_EP, 0xDEADBEEF);
            }
            let sgate = SGateObject::new(&rgate, self.id() as tcu::Label, 1);
            pemux.config_snd_ep(self.eps_start + tcu::SYSC_SEP_OFF, vpe, &sgate.borrow())?;
        }

        // attach syscall receive endpoint
        let mut rbuf_addr = self.rbuf_phys;
        {
            let rgate = RGateObject::new(cfg::SYSC_RBUF_ORD, cfg::SYSC_RBUF_ORD);
            let mut rgate = rgate.borrow_mut();
            rgate.activate(self.pe_id(), self.eps_start + tcu::SYSC_REP_OFF, rbuf_addr);
            pemux.config_rcv_ep(
                self.eps_start + tcu::SYSC_REP_OFF,
                vpe,
                None,
                &mut rgate,
            )?;
            rbuf_addr += cfg::SYSC_RBUF_SIZE as goff;
        }

        // attach upcall receive endpoint
        {
            let rgate = RGateObject::new(cfg::UPCALL_RBUF_ORD, cfg::UPCALL_RBUF_ORD);
            let mut rgate = rgate.borrow_mut();
            rgate.activate(
                self.pe_id(),
                self.eps_start + tcu::UPCALL_REP_OFF,
                rbuf_addr,
            );
            pemux.config_rcv_ep(
                self.eps_start + tcu::UPCALL_REP_OFF,
                vpe,
                Some(self.eps_start + tcu::UPCALL_RPLEP_OFF),
                &mut rgate,
            )?;
            rbuf_addr += cfg::UPCALL_RBUF_SIZE as goff;
        }

        // attach default receive endpoint
        {
            let rgate = RGateObject::new(cfg::DEF_RBUF_ORD, cfg::DEF_RBUF_ORD);
            let mut rgate = rgate.borrow_mut();
            rgate.activate(self.pe_id(), self.eps_start + tcu::DEF_REP_OFF, rbuf_addr);
            pemux.config_rcv_ep(
                self.eps_start + tcu::DEF_REP_OFF,
                vpe,
                None,
                &mut rgate,
            )?;
        }

        Ok(())
    }

    pub fn id(&self) -> VPEId {
        self.id
    }

    pub fn pe(&self) -> Rc<RefCell<PEObject>> {
        self.pe.clone()
    }

    pub fn pe_id(&self) -> PEId {
        self.pe.borrow().pe()
    }

    pub fn pe_desc(&self) -> PEDesc {
        platform::pe_desc(self.pe_id())
    }

    pub fn rbuf_addr(&self) -> goff {
        self.rbuf_phys
    }

    pub fn eps_start(&self) -> EpId {
        self.eps_start
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn obj_caps(&self) -> &CapTable {
        &self.obj_caps
    }

    pub fn obj_caps_mut(&mut self) -> &mut CapTable {
        &mut self.obj_caps
    }

    pub fn map_caps(&self) -> &CapTable {
        &self.map_caps
    }

    pub fn map_caps_mut(&mut self) -> &mut CapTable {
        &mut self.map_caps
    }

    pub fn state(&self) -> State {
        self.state.clone()
    }

    pub fn set_state(&mut self, state: State) {
        self.state = state;
    }

    pub fn has_app(&self) -> bool {
        self.flags.contains(VPEFlags::HASAPP)
    }

    pub fn is_bootmod(&self) -> bool {
        self.flags.contains(VPEFlags::BOOTMOD)
    }

    pub fn set_mem_base(&mut self, addr: goff) {
        pemng::get().pemux(self.pe_id()).set_mem_base(addr);
    }

    pub fn first_sel(&self) -> CapSel {
        self.first_sel
    }

    pub fn set_first_sel(&mut self, sel: CapSel) {
        self.first_sel = sel;
    }

    pub fn pid(&self) -> i32 {
        self.pid
    }

    pub fn set_pid(&mut self, pid: i32) {
        self.pid = pid;
    }

    pub fn fetch_exit_code(&mut self) -> Option<i32> {
        mem::replace(&mut self.exit_code, None)
    }

    pub fn wait() {
        let event = &EXIT_EVENT as *const _ as thread::Event;
        thread::ThreadManager::get().wait_for(event);
    }

    fn check_exits(vpe: &Rc<RefCell<VPE>>, reply: &mut kif::syscalls::VPEWaitReply) -> bool {
        {
            let vpe = vpe.borrow();
            for sel in &vpe.wait_sels {
                let wvpe = vpe.obj_caps().get(*sel as CapSel).map(|c| c.get().clone());
                match wvpe {
                    Some(KObject::VPE(wv)) => {
                        if wv.borrow().id() == vpe.id() {
                            continue;
                        }

                        if let Some(code) = wv.borrow_mut().fetch_exit_code() {
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
        vpe: &Rc<RefCell<VPE>>,
        sels: &[u64],
        reply: &mut kif::syscalls::VPEWaitReply,
    ) -> bool {
        let was_empty = vpe.borrow().wait_sels.len() == 0;

        vpe.borrow_mut().wait_sels.clear();
        vpe.borrow_mut().wait_sels.extend_from_slice(sels);

        if !was_empty {
            return false;
        }

        loop {
            if Self::check_exits(vpe, reply) {
                break;
            }
        }

        vpe.borrow_mut().wait_sels.clear();
        true
    }

    pub fn upcall_vpewait(&mut self, event: u64, reply: &kif::syscalls::VPEWaitReply) {
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
            .send(self.eps_start + UPCALL_REP_OFF, 0, util::object_to_bytes(&msg))
            .unwrap();
    }

    pub fn start_app(vpe: &Rc<RefCell<VPE>>, pid: i32) -> Result<(), Error> {
        if !vpe.borrow().flags.contains(VPEFlags::HASAPP) {
            return Ok(());
        }

        {
            let mut vpe_mut = vpe.borrow_mut();
            vpe_mut.pid = pid;
            vpe_mut.flags |= VPEFlags::HASAPP;
        }

        pemng::get().start_vpe(vpe)
    }

    pub fn stop_app(vpe: Rc<RefCell<VPE>>, exit_code: i32, is_self: bool) {
        if !vpe.borrow().flags.contains(VPEFlags::HASAPP) {
            return;
        }

        klog!(VPES, "Stopping VPE {} [id={}]", vpe.borrow().name(), vpe.borrow().id());

        if is_self {
            Self::exit_app(vpe, exit_code);
        }
        else {
            ktcu::drop_msgs(ktcu::KSYS_EP, vpe.borrow().id() as Label);
            if vpe.borrow().state == State::RUNNING {
                // devices always exit successfully
                let exit_code = if vpe.borrow().pe_desc().is_device() { 0 } else { 1 };
                Self::exit_app(vpe, exit_code);
            }
            else {
                vpe.borrow_mut().flags.remove(VPEFlags::HASAPP);
                pemng::get().stop_vpe(&vpe, false, true).unwrap();
            }
        }
    }

    fn exit_app(vpe: Rc<RefCell<VPE>>, exit_code: i32) {
        // TODO force-invalidate all EPs of this VPE

        {
            let mut vpe_mut = vpe.borrow_mut();
            vpe_mut.flags.remove(VPEFlags::HASAPP);
            vpe_mut.exit_code = Some(exit_code);
        }

        pemng::get().stop_vpe(&vpe, false, false).unwrap();

        vpe.borrow_mut().revoke_caps(false);

        let event = &EXIT_EVENT as *const _ as thread::Event;
        thread::ThreadManager::get().notify(event, None);

        // if it's a boot module, there is nobody waiting for it; just remove it
        if vpe.borrow().is_bootmod() {
            let id = vpe.borrow().id();
            vpemng::get().remove(id);
        }
    }

    pub fn destroy(&mut self) {
        self.state = State::DEAD;

        self.obj_caps.revoke_all(true);
        self.map_caps.revoke_all(true);
    }

    fn revoke_caps(&mut self, all: bool) {
        self.obj_caps.revoke_all(all);
        self.map_caps.revoke_all(true);
    }

    pub fn revoke(vpe: &Rc<RefCell<VPE>>, crd: CapRngDesc, own: bool) {
        // we can't use borrow_mut() here, because revoke might need to use borrow as well.
        unsafe {
            if crd.cap_type() == CapType::OBJECT {
                (*vpe.as_ptr()).obj_caps_mut().revoke(crd, own);
            }
            else {
                (*vpe.as_ptr()).map_caps_mut().revoke(crd, own);
            }
        }
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
