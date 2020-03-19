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

use base::boxed::Box;
use base::cell::StaticCell;
use base::cfg;
use base::col::BoxList;
use base::errors::Error;
use base::goff;
use base::kif;
use base::math;
use base::tcu;
use base::util;
use core::ptr::NonNull;

use arch::{set_entry_sp, State};
use helper;
use paging::Allocator;
use vma::PfState;

struct PTAllocator {
    vpe: u64,
    pts_start: paging::MMUPTE,
    pts_count: usize,
    pts_pos: usize,
}

impl Allocator for PTAllocator {
    fn allocate_pt(&mut self) -> paging::MMUPTE {
        assert!(self.vpe != kif::pemux::IDLE_ID);
        if self.pts_pos < self.pts_count {
            let res = self.pts_start + (cfg::PAGE_SIZE * self.pts_pos) as paging::MMUPTE;
            self.pts_pos += 1;
            res
        }
        else {
            0
        }
    }

    fn translate_pt(&self, phys: paging::MMUPTE) -> usize {
        let pts_end = self.pts_start + (self.pts_count * cfg::PAGE_SIZE) as paging::MMUPTE;
        assert!(phys >= self.pts_start && phys < pts_end);
        let off = phys - self.pts_start;
        if *BOOTSTRAP {
            off as usize
        }
        else {
            cfg::PE_MEM_BASE + off as usize
        }
    }
}

struct Info {
    pe_id: u64,
    pe_desc: kif::PEDesc,
    mem_start: u64,
    mem_end: u64,
}

#[derive(PartialEq, Eq)]
enum VPEState {
    Running,
    Ready,
    Blocked,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ScheduleAction {
    TryBlock,
    Block,
    Preempt,
    Kill,
}

pub struct VPE {
    state: VPEState,
    prev: Option<NonNull<VPE>>,
    next: Option<NonNull<VPE>>,
    aspace: paging::AddrSpace<PTAllocator>,
    user_state: State,
    user_state_addr: usize,
    vpe_reg: tcu::Reg,
    eps_start: tcu::EpId,
    cmd: helper::TCUCmdState,
    pf_state: Option<PfState>,
    cont: Option<fn() -> bool>,
}

impl_boxitem!(VPE);

static VPES: StaticCell<[Option<NonNull<VPE>>; 64]> = StaticCell::new([None; 64]);

static IDLE: StaticCell<Option<Box<VPE>>> = StaticCell::new(None);
static OUR: StaticCell<Option<Box<VPE>>> = StaticCell::new(None);

static CUR: StaticCell<Option<Box<VPE>>> = StaticCell::new(None);
static RDY: StaticCell<BoxList<VPE>> = StaticCell::new(BoxList::new());
static BLK: StaticCell<BoxList<VPE>> = StaticCell::new(BoxList::new());

// TODO for some reason, we need to put that in a separate struct than INFO, because otherwise
// wrong code is generated for ARM.
static BOOTSTRAP: StaticCell<bool> = StaticCell::new(true);
static INFO: StaticCell<Info> = StaticCell::new(Info {
    pe_id: 0,
    pe_desc: kif::PEDesc::new_from(0),
    mem_start: 0,
    mem_end: 0,
});

pub fn init(pe_id: u64, pe_desc: kif::PEDesc, mem_start: u64, mem_size: u64) {
    INFO.get_mut().pe_id = pe_id;
    INFO.get_mut().pe_desc = pe_desc;
    INFO.get_mut().mem_start = mem_start;
    INFO.get_mut().mem_end = mem_start + mem_size;

    let root_pt = mem_start + cfg::PAGE_SIZE as u64;
    let pts_count = mem_size as usize / cfg::PAGE_SIZE;
    IDLE.set(Some(Box::new(VPE::new(
        kif::pemux::IDLE_ID,
        0,
        root_pt,
        mem_start,
        pts_count,
    ))));
    OUR.set(Some(Box::new(VPE::new(
        kif::pemux::VPE_ID,
        0,
        root_pt,
        mem_start,
        pts_count,
    ))));

    idle().state = VPEState::Ready;
    unsafe {
        RDY.get_mut().push_back(Box::from_raw(idle()));
    }

    if pe_desc.has_virtmem() {
        our().init();
        our().switch_to();
        paging::enable_paging();
    }

    BOOTSTRAP.set(false);
}

pub fn add(id: u64, eps_start: tcu::EpId) {
    log!(crate::LOG_VPES, "Created VPE {}", id);

    // TODO temporary
    let pt_begin = INFO.get().mem_start + (INFO.get().mem_end - INFO.get().mem_start) / 2;
    let root_pt = pt_begin;
    let pts_count = (INFO.get().mem_end - INFO.get().mem_start) as usize / cfg::PAGE_SIZE;
    let mut vpe = Box::new(VPE::new(
        id,
        eps_start,
        root_pt,
        INFO.get().mem_start,
        pts_count,
    ));

    if INFO.get().pe_desc.has_virtmem() {
        vpe.init();
    }

    unsafe {
        VPES.get_mut()[id as usize] = Some(NonNull::new_unchecked(vpe.as_mut()));
    }
    BLK.get_mut().push_back(vpe);
}

pub fn get_mut(id: u64) -> Option<&'static mut VPE> {
    if id == kif::pemux::VPE_ID {
        return Some(our());
    }
    else {
        VPES.get_mut()[id as usize]
            .as_mut()
            .map(|v| unsafe { v.as_mut() })
    }
}

pub fn our() -> &'static mut VPE {
    OUR.get_mut().as_mut().unwrap()
}

pub fn idle() -> &'static mut VPE {
    IDLE.get_mut().as_mut().unwrap()
}

pub fn try_cur() -> Option<&'static mut Box<VPE>> {
    CUR.get_mut().as_mut()
}

pub fn cur() -> &'static mut VPE {
    try_cur().unwrap()
}

pub fn schedule(mut state_addr: usize, mut action: ScheduleAction) -> usize {
    loop {
        let mut next = RDY.get_mut().pop_front().unwrap();

        log!(crate::LOG_VPES, "Switching to VPE {}", next.id());

        // make current
        let old_id = tcu::TCU::xchg_vpe(next.vpe_reg());
        let old = try_cur();

        // if there are messages left and we try to block the VPE, don't schedule
        if action == ScheduleAction::TryBlock && (old_id >> 3) & 0xFFFF != 0 {
            let next_id = tcu::TCU::xchg_vpe(old_id);
            next.set_vpe_reg(next_id);
            RDY.get_mut().push_back(next);
            return state_addr;
        }

        if let Some(old) = old {
            // don't do that if we're switching away from a continuation (see below)
            if state_addr != 0 {
                // save TCU command registers
                old.cmd.save();
                // remember state to resume for later
                if old.user_state_addr != 0 {
                    old.user_state_addr = state_addr;
                }
            }
            old.set_vpe_reg(old_id);
        }

        // change address space
        next.switch_to();

        // set SP for the next entry
        let new_state = next.user_state_addr;
        set_entry_sp(new_state + util::size_of::<State>());
        let cont = next.cont.take();
        next.state = VPEState::Running;

        if cont.is_none() {
            // restore TCU command registers
            next.cmd.restore();
        }

        // exchange CUR
        if let Some(mut old) = CUR.set(Some(next)) {
            log!(crate::LOG_VPES, "{:?} VPE {}", action, old.id());

            // block, preempt or kill VPE
            match action {
                ScheduleAction::TryBlock | ScheduleAction::Block => {
                    old.state = VPEState::Blocked;
                    BLK.get_mut().push_back(old);
                },
                ScheduleAction::Preempt => {
                    old.state = VPEState::Ready;
                    RDY.get_mut().push_back(old);
                },
                ScheduleAction::Kill => {
                    VPES.get_mut()[old.id() as usize] = None;
                },
            }
        }

        // if there is a function to call, do that
        if let Some(f) = cont {
            // only resume this VPE if it has been initialized
            let finished = f();
            if finished && new_state != 0 {
                cur().cmd.restore();
                break new_state;
            }
            // in this case, the VPE is not ready, so block it and don't set the state addr again
            action = ScheduleAction::Block;
            state_addr = 0;
            if !finished {
                // set the continuation again
                cur().cont = Some(f);
            }
        }
        else {
            break new_state;
        }
    }
}

pub fn remove_cur(status: u32) {
    remove(cur().id(), status, true);
}

pub fn remove(id: u64, status: u32, notify: bool) {
    if let Some(v) = VPES.get_mut()[id as usize].take() {
        let old = match unsafe { &v.as_ref().state } {
            VPEState::Running => CUR.set(None).unwrap(),
            VPEState::Ready => RDY.get_mut().remove_if(|v| v.id() == id).unwrap(),
            VPEState::Blocked => BLK.get_mut().remove_if(|v| v.id() == id).unwrap(),
        };

        log!(crate::LOG_VPES, "Destroyed VPE {}", old.id());

        if notify {
            // change to our VPE (no need to save old vpe_reg; VPE is dead)
            let pex_is_running = tcu::TCU::get_cur_vpe() >> 19 == kif::pemux::VPE_ID;
            if !pex_is_running {
                tcu::TCU::xchg_vpe(our().vpe_reg());
            }

            let msg = &mut crate::msgs_mut().exit_notify;
            msg.op = kif::pemux::Calls::EXIT.val as u64;
            msg.vpe_sel = old.id();
            msg.code = status as u64;

            let msg_addr = msg as *const _ as *const u8;
            let size = util::size_of::<kif::pemux::Exit>();
            tcu::TCU::send(tcu::KPEX_SEP, msg_addr, size, 0, tcu::NO_REPLIES).unwrap();

            // switch back to old VPE
            if !pex_is_running {
                let our_vpe = tcu::TCU::xchg_vpe(old.vpe_reg());
                our().set_vpe_reg(our_vpe);
            }
        }

        if old.state == VPEState::Running {
            crate::reg_scheduling(ScheduleAction::Kill);
        }
    }
}

impl VPE {
    pub fn new(
        id: u64,
        eps_start: tcu::EpId,
        root_pt: goff,
        pts_start: goff,
        pts_count: usize,
    ) -> Self {
        let allocator = PTAllocator {
            vpe: id,
            pts_start: paging::noc_to_phys(pts_start) as paging::MMUPTE,
            pts_count,
            // + 1 to skip the root PT
            pts_pos: (root_pt - pts_start) as usize / cfg::PAGE_SIZE + 1,
        };

        VPE {
            prev: None,
            next: None,
            aspace: paging::AddrSpace::new(id, root_pt, allocator, false),
            vpe_reg: id << 19,
            state: VPEState::Blocked,
            user_state: State::default(),
            user_state_addr: 0,
            eps_start,
            cmd: helper::TCUCmdState::new(),
            pf_state: None,
            cont: None,
        }
    }

    pub fn map(
        &mut self,
        virt: usize,
        phys: goff,
        pages: usize,
        perm: kif::PageFlags,
    ) -> Result<(), Error> {
        self.aspace.map_pages(virt, phys, pages, perm)
    }

    pub fn translate(&self, virt: usize, perm: kif::PageFlags) -> kif::PTE {
        self.aspace.translate(virt, perm.bits())
    }

    pub fn id(&self) -> u64 {
        self.aspace.id()
    }

    pub fn vpe_reg(&self) -> tcu::Reg {
        self.vpe_reg
    }

    pub fn set_vpe_reg(&mut self, val: tcu::Reg) {
        self.vpe_reg = val;
    }

    pub fn eps_start(&self) -> tcu::EpId {
        self.eps_start
    }

    pub fn msgs(&self) -> u16 {
        ((self.vpe_reg >> 3) & 0xFFFF) as u16
    }

    pub fn has_msgs(&self) -> bool {
        self.msgs() != 0
    }

    pub fn add_msg(&mut self) {
        self.vpe_reg += 1 << 3;
    }

    pub fn rem_msgs(&mut self, count: u16) {
        assert!(self.msgs() >= count);
        self.vpe_reg -= (count as u64) << 3;
    }

    pub fn block(&mut self, action: ScheduleAction, cont: Option<fn() -> bool>) {
        self.cont = cont;
        if self.state == VPEState::Running {
            crate::reg_scheduling(action);
        }
    }

    pub fn unblock(&mut self) {
        if self.state == VPEState::Blocked {
            let mut vpe = BLK.get_mut().remove_if(|v| v.id() == self.id()).unwrap();
            vpe.state = VPEState::Ready;
            RDY.get_mut().push_back(vpe);
            crate::reg_scheduling(ScheduleAction::Preempt);
        }
    }

    pub fn start_pf(&mut self, pf_state: PfState) {
        self.pf_state = Some(pf_state);
    }

    pub fn finish_pf(&mut self) -> (u64, PfState) {
        (self.cmd.xfer_buf(), self.pf_state.take().unwrap())
    }

    pub fn start(&mut self, state_addr: usize) {
        assert!(self.user_state_addr == 0);
        if self.id() != kif::pemux::IDLE_ID {
            // remember the current PE
            crate::env().pe_id = INFO.pe_id;
            self.user_state
                .init(::env().entry as usize, ::env().sp as usize);
            self.user_state_addr = &self.user_state as *const _ as usize;
        }
        else {
            self.user_state_addr = state_addr;
        }
    }

    pub fn switch_to(&self) {
        if INFO.get().pe_desc.has_virtmem() {
            self.aspace.switch_to();
        }
    }

    fn init(&mut self) {
        extern "C" {
            static _text_start: u8;
            static _text_end: u8;
            static _data_start: u8;
            static _data_end: u8;
            static _bss_start: u8;
            static _bss_end: u8;
        }

        // we have to perform the initialization here, because it calls xlate_pt(), so that the VPE
        // needs to be accessible via get_mut().
        self.aspace.init();

        // map TCU
        let rw = kif::PageFlags::RW;
        self.map(
            tcu::MMIO_ADDR,
            tcu::MMIO_ADDR as goff,
            tcu::MMIO_SIZE / cfg::PAGE_SIZE,
            kif::PageFlags::U | rw,
        )
        .unwrap();
        self.map(
            tcu::MMIO_PRIV_ADDR,
            tcu::MMIO_PRIV_ADDR as goff,
            tcu::MMIO_PRIV_SIZE / cfg::PAGE_SIZE,
            rw,
        )
        .unwrap();

        // map text, data, and bss
        let rx = kif::PageFlags::RX;
        unsafe {
            self.map_segment(&_text_start, &_text_end, rx);
            self.map_segment(&_data_start, &_data_end, rw);
            self.map_segment(&_bss_start, &_bss_end, rw);
        }

        // map receive buffers
        if self.id() == kif::pemux::VPE_ID {
            self.map_rbuf(
                cfg::PEMUX_RBUF_SPACE,
                cfg::PEMUX_RBUF_SIZE,
                kif::PageFlags::R,
            );
        }
        else {
            // map our own receive buffer again
            let pte = our().translate(cfg::PEMUX_RBUF_SPACE, kif::PageFlags::R);
            self.map(
                cfg::PEMUX_RBUF_SPACE,
                pte & !cfg::PAGE_MASK as goff,
                cfg::PEMUX_RBUF_SIZE / cfg::PAGE_SIZE,
                kif::PageFlags::R,
            )
            .unwrap();

            // map application receive buffer
            let perm = kif::PageFlags::R | kif::PageFlags::U;
            self.map_rbuf(cfg::RECVBUF_SPACE, cfg::RECVBUF_SIZE, perm);
        }

        // map PTs
        let noc_begin = paging::phys_to_noc(self.aspace.allocator().pts_start as u64);
        self.map(
            cfg::PE_MEM_BASE,
            noc_begin,
            self.aspace.allocator().pts_count,
            kif::PageFlags::RW,
        )
        .unwrap();

        // map vectors
        #[cfg(target_arch = "arm")]
        self.map(0, noc_begin, 1, rx).unwrap();

        // insert fixed entry for messages into TLB
        let virt = crate::msgs_mut() as *mut _ as usize;
        let pte = self.translate(virt, kif::PageFlags::R);
        let phys = pte & !(cfg::PAGE_MASK as u64);
        let mut flags = kif::PageFlags::from_bits_truncate(pte & cfg::PAGE_MASK as u64);
        flags |= kif::PageFlags::FIXED;
        tcu::TCU::insert_tlb(self.id() as u16, virt, phys, flags);
    }

    fn map_rbuf(&mut self, addr: usize, size: usize, perm: kif::PageFlags) {
        for i in 0..(size / cfg::PAGE_SIZE) {
            let frame = self.aspace.allocator_mut().allocate_pt();
            assert!(frame != 0);
            self.map(
                addr + i * cfg::PAGE_SIZE,
                paging::phys_to_noc(frame as u64),
                1,
                perm,
            )
            .unwrap();
        }
    }

    fn map_segment(&mut self, start: *const u8, end: *const u8, perm: kif::PageFlags) {
        let start = math::round_dn(start as usize, cfg::PAGE_SIZE);
        let end = math::round_up(end as usize, cfg::PAGE_SIZE);
        let pages = (end - start) / cfg::PAGE_SIZE;
        self.map(
            start,
            paging::phys_to_noc((self.aspace.allocator().pts_start as usize + start) as goff),
            pages,
            perm,
        )
        .unwrap();
    }
}
