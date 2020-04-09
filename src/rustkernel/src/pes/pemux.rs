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

use base::col::{BitVec, Vec};
use base::errors::{Code, Error};
use base::goff;
use base::kif;
use base::mem::GlobAddr;
use base::rc::Rc;
use base::tcu::{self, EpId, PEId};

use cap::{MGateObject, PEObject, RGateObject, SGateObject};
use com::SendQueue;
use ktcu;
use pes::{VPEId, INVAL_ID};
use platform;

pub const MSG_ORD: u32 = 7;

pub struct PEMux {
    pe: Rc<PEObject>,
    vpes: Vec<VPEId>,
    queue: SendQueue,
    eps: BitVec,
    mem_base: goff,
}

impl PEMux {
    pub fn new(pe: PEId) -> Self {
        let mut pemux = PEMux {
            pe: PEObject::new(pe, (tcu::EP_COUNT - tcu::FIRST_USER_EP) as u32),
            vpes: Vec::new(),
            queue: SendQueue::new(pe as u64, pe),
            eps: BitVec::new(tcu::EP_COUNT),
            mem_base: 0,
        };

        for ep in 0..tcu::FIRST_USER_EP {
            pemux.eps.set(ep);
        }

        #[cfg(target_os = "none")]
        if platform::pe_desc(pe).supports_pemux() {
            pemux.init();
        }

        pemux
    }

    pub fn has_vpes(&self) -> bool {
        !self.vpes.is_empty()
    }

    pub fn add_vpe(&mut self, vpe: VPEId) {
        self.vpes.push(vpe);
    }

    pub fn rem_vpe(&mut self, vpe: VPEId) {
        assert!(self.vpes.len() > 0);
        self.vpes.retain(|id| *id != vpe);
    }

    #[cfg(target_os = "none")]
    fn init(&mut self) {
        use base::cfg;

        // configure send EP
        {
            ktcu::config_remote_ep(self.pe_id(), tcu::KPEX_SEP, |regs| {
                ktcu::config_send(
                    regs,
                    kif::pemux::VPE_ID as VPEId,
                    self.pe_id() as tcu::Label,
                    platform::kernel_pe(),
                    ktcu::KPEX_EP,
                    cfg::KPEX_RBUF_ORD,
                    1,
                );
            })
            .unwrap();
        }

        // configure receive EP
        let mut rbuf = platform::rbuf_pemux(self.pe_id());
        {
            ktcu::config_remote_ep(self.pe_id(), tcu::KPEX_REP, |regs| {
                ktcu::config_recv(
                    regs,
                    kif::pemux::VPE_ID as VPEId,
                    rbuf,
                    cfg::KPEX_RBUF_ORD,
                    cfg::KPEX_RBUF_ORD,
                    None,
                );
            })
            .unwrap();
            rbuf += 1 << cfg::KPEX_RBUF_ORD;
        }

        // configure upcall EP
        {
            ktcu::config_remote_ep(self.pe_id(), tcu::PEXUP_REP, |regs| {
                ktcu::config_recv(
                    regs,
                    kif::pemux::VPE_ID as VPEId,
                    rbuf,
                    cfg::PEXUP_RBUF_ORD,
                    cfg::PEXUP_RBUF_ORD,
                    Some(tcu::PEXUP_RPLEP),
                );
            })
            .unwrap();
        }
    }

    pub fn pe(&self) -> &Rc<PEObject> {
        &self.pe
    }

    pub fn pe_id(&self) -> PEId {
        self.pe.pe()
    }

    #[cfg(target_os = "linux")]
    pub fn eps_base(&mut self) -> goff {
        self.mem_base
    }

    pub fn set_mem_base(&mut self, addr: goff) {
        self.mem_base = addr;
    }

    pub fn find_eps(&self, count: u32) -> Result<tcu::EpId, Error> {
        let mut start = self.eps.first_clear();
        let mut bit = start;
        while bit < start + count as usize && bit < tcu::EP_COUNT {
            if self.eps.is_set(bit) {
                start = bit + 1;
            }
            bit += 1;
        }

        if bit != start + count as usize {
            Err(Error::new(Code::NoSpace))
        }
        else {
            Ok(start)
        }
    }

    pub fn eps_free(&self, start: tcu::EpId, count: u32) -> bool {
        for ep in start..start + count as usize {
            if self.eps.is_set(ep) {
                return false;
            }
        }
        true
    }

    pub fn alloc_eps(&mut self, start: tcu::EpId, count: u32) {
        klog!(
            EPS,
            "PEMux[{}] allocating EPS {}..{}",
            self.pe_id(),
            start,
            start as u32 + count - 1
        );
        for bit in start..start + count as usize {
            assert!(!self.eps.is_set(bit));
            self.eps.set(bit);
        }
    }

    pub fn free_eps(&mut self, start: tcu::EpId, count: u32) {
        klog!(
            EPS,
            "PEMux[{}] freeing EPS {}..{}",
            self.pe_id(),
            start,
            start as u32 + count - 1
        );
        for bit in start..start + count as usize {
            assert!(self.eps.is_set(bit));
            self.eps.clear(bit);
        }
    }

    fn ep_vpe_id(&self, vpe: VPEId) -> VPEId {
        match platform::is_shared(self.pe_id()) {
            true => vpe,
            false => INVAL_ID,
        }
    }

    pub fn config_snd_ep(
        &mut self,
        ep: EpId,
        vpe: VPEId,
        obj: &Rc<SGateObject>,
    ) -> Result<(), Error> {
        let rgate = obj.rgate();
        assert!(rgate.activated());

        klog!(EPS, "PE{}:EP{} = {:?}", self.pe_id(), ep, obj);

        ktcu::config_remote_ep(self.pe_id(), ep, |regs| {
            let vpe = self.ep_vpe_id(vpe);
            ktcu::config_send(
                regs,
                vpe,
                obj.label(),
                rgate.pe().unwrap(),
                rgate.ep().unwrap(),
                rgate.msg_order(),
                obj.credits(),
            );
        })
    }

    pub fn config_rcv_ep(
        &mut self,
        ep: EpId,
        vpe: VPEId,
        reply_eps: Option<EpId>,
        obj: &Rc<RGateObject>,
    ) -> Result<(), Error> {
        klog!(EPS, "PE{}:EP{} = {:?}", self.pe_id(), ep, obj);

        ktcu::config_remote_ep(self.pe_id(), ep, |regs| {
            let vpe = self.ep_vpe_id(vpe);
            ktcu::config_recv(
                regs,
                vpe,
                obj.addr(),
                obj.order(),
                obj.msg_order(),
                reply_eps,
            );
        })?;

        thread::ThreadManager::get().notify(obj.get_event(), None);
        Ok(())
    }

    pub fn config_mem_ep(
        &mut self,
        ep: EpId,
        vpe: VPEId,
        obj: &Rc<MGateObject>,
        pe_id: PEId,
        off: goff,
    ) -> Result<(), Error> {
        if off >= obj.size() as goff || obj.addr().raw().checked_add(off).is_none() {
            return Err(Error::new(Code::InvArgs));
        }

        klog!(EPS, "PE{}:EP{} = {:?}", self.pe_id(), ep, obj);

        ktcu::config_remote_ep(self.pe_id(), ep, |regs| {
            let vpe = self.ep_vpe_id(vpe);
            ktcu::config_mem(
                regs,
                vpe,
                pe_id,
                obj.offset() + off,
                (obj.size() - off) as usize,
                obj.perms(),
            );
        })
    }

    pub fn invalidate_ep(&mut self, ep: EpId, force: bool) -> Result<(), Error> {
        klog!(EPS, "PE{}:EP{} = invalid", self.pe_id(), ep);

        ktcu::invalidate_ep_remote(self.pe_id(), ep, force)
    }

    #[cfg(target_os = "none")]
    pub fn handle_call(&mut self, msg: &tcu::Message) {
        use pes::{vpemng, VPE};

        let req = msg.get_data::<kif::pemux::Exit>();
        let vpe_id = req.vpe_sel as VPEId;
        let exitcode = req.code as i32;

        klog!(
            PEXC,
            "PEMux[{}] got exit(vpe={}, code={})",
            self.pe_id(),
            vpe_id,
            exitcode
        );

        if self.vpes.contains(&vpe_id) {
            let vpe = vpemng::get().vpe(vpe_id).unwrap();
            VPE::stop_app(&vpe, exitcode, true);
        }

        let reply = kif::DefaultReply { error: 0 };
        ktcu::reply(ktcu::KPEX_EP, &reply, msg).unwrap();
    }

    #[cfg(target_os = "linux")]
    pub fn vpe_ctrl(
        &mut self,
        _vpe: VPEId,
        _eps_start: EpId,
        _ctrl: base::kif::pemux::VPEOp,
    ) -> Result<(), Error> {
        // nothing to do
        Ok(())
    }

    #[cfg(target_os = "none")]
    pub fn vpe_ctrl(
        &mut self,
        vpe: VPEId,
        eps_start: EpId,
        ctrl: base::kif::pemux::VPEOp,
    ) -> Result<(), Error> {
        let req = kif::pemux::VPECtrl {
            op: kif::pemux::Upcalls::VPE_CTRL.val as u64,
            vpe_sel: vpe as u64,
            vpe_op: ctrl.val as u64,
            eps_start: eps_start as u64,
        };

        klog!(
            PEXC,
            "PEMux[{}] sending VPECtrl(vpe={}, ctrl={:?})",
            self.pe_id(),
            vpe,
            ctrl
        );

        self.upcall(&req).map(|_| ())
    }

    #[cfg(target_os = "linux")]
    pub fn map(
        &mut self,
        _vpe: VPEId,
        _virt: goff,
        _glob: GlobAddr,
        _pages: usize,
        _perm: kif::PageFlags,
    ) -> Result<(), Error> {
        Ok(())
    }

    #[cfg(target_os = "none")]
    pub fn map(
        &mut self,
        vpe: VPEId,
        virt: goff,
        glob: GlobAddr,
        pages: usize,
        perm: kif::PageFlags,
    ) -> Result<(), Error> {
        let req = kif::pemux::Map {
            op: kif::pemux::Upcalls::MAP.val as u64,
            vpe_sel: vpe as u64,
            virt: virt as u64,
            global: glob.raw(),
            pages: pages as u64,
            perm: perm.bits() as u64,
        };

        klog!(
            PEXC,
            "PEMux[{}] sending Map(vpe={}, virt={:#x}, glob={:?}, pages={}, perm={:?})",
            self.pe_id(),
            vpe,
            virt,
            glob,
            pages,
            perm
        );

        self.upcall(&req).map(|_| ())
    }

    #[cfg(target_os = "none")]
    pub fn translate(
        &mut self,
        vpe: VPEId,
        virt: goff,
        perm: kif::Perm,
    ) -> Result<GlobAddr, Error> {
        use base::cfg::PAGE_MASK;

        let req = kif::pemux::Translate {
            op: kif::pemux::Upcalls::TRANSLATE.val as u64,
            vpe_sel: vpe as u64,
            virt: virt as u64,
            perm: perm.bits() as u64,
        };

        klog!(
            PEXC,
            "PEMux[{}] sending Translate(vpe={}, virt={:#x})",
            self.pe_id(),
            vpe,
            virt
        );

        self.upcall(&req)
            .map(|reply| GlobAddr::new(reply.val & !PAGE_MASK as goff))
    }

    #[cfg(target_os = "none")]
    fn upcall<R>(&mut self, req: &R) -> Result<&'static kif::pemux::Response, Error> {
        use base::util;

        let event = self
            .queue
            .send(tcu::PEXUP_REP, 0, util::object_to_bytes(req))?;
        thread::ThreadManager::get().wait_for(event);

        let reply = thread::ThreadManager::get().fetch_msg().unwrap();
        let reply = reply.get_data::<kif::pemux::Response>();
        if reply.error == 0 {
            Ok(reply)
        }
        else {
            Err(Error::new(Code::from(reply.error as u32)))
        }
    }

    #[cfg(target_os = "linux")]
    pub fn update_eps(&mut self) -> Result<(), Error> {
        ktcu::update_eps(self.pe_id(), self.mem_base)
    }
}
