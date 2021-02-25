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
use base::cell::{LazyStaticCell, StaticCell};
use base::cfg;
use base::col::{BoxList, Vec};
use base::errors::{Code, Error};
use base::goff;
use base::impl_boxitem;
use base::kif;
use base::log;
use base::math;
use base::mem::{size_of, GlobAddr};
use base::tcu;
use core::cmp;
use core::ptr::NonNull;
use paging::{Allocator, Phys};

use crate::arch;
use crate::helper;
use crate::pex_env;
use crate::timer::{self, Nanos};
use crate::vma::PfState;

pub type Id = paging::VPEId;

const TIME_SLICE: Nanos = 1_000_000;

struct PTAllocator {
    vpe: Id,
}

impl Allocator for PTAllocator {
    fn allocate_pt(&mut self) -> Result<Phys, Error> {
        assert!(self.vpe != kif::pemux::IDLE_ID);
        if let Some(pt) = PTS.get_mut().pop() {
            log!(crate::LOG_PTS, "Alloc PT {:#x} (free: {})", pt, PTS.len());
            Ok(pt)
        }
        else {
            Err(Error::new(Code::NoSpace))
        }
    }

    fn translate_pt(&self, phys: Phys) -> usize {
        if *BOOTSTRAP {
            phys as usize
        }
        else {
            cfg::PE_MEM_BASE + (phys as usize - cfg::MEM_OFFSET)
        }
    }

    fn free_pt(&mut self, phys: Phys) {
        log!(crate::LOG_PTS, "Free PT {:#x} (free: {})", phys, PTS.len());
        PTS.get_mut().push(phys);
    }
}

#[derive(PartialEq, Eq)]
enum VPEState {
    Running,
    Ready,
    Blocked,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ScheduleAction {
    Block,
    Yield,
    Preempt,
    Kill,
}

#[derive(PartialEq, Eq)]
pub enum ContResult {
    Waiting,
    Success,
    Failure,
}

pub struct VPE {
    state: VPEState,
    prev: Option<NonNull<VPE>>,
    next: Option<NonNull<VPE>>,
    aspace: Option<paging::AddrSpace<PTAllocator>>,
    frames: Vec<Phys>,
    #[cfg(any(target_arch = "riscv64", target_arch = "x86_64"))]
    fpu_state: arch::FPUState,
    user_state: arch::State,
    user_state_addr: usize,
    sleeping: bool,
    scheduled: Nanos,
    budget_total: Nanos,
    budget_left: Nanos,
    wait_ep: Option<tcu::EpId>,
    vpe_reg: tcu::Reg,
    eps_start: tcu::EpId,
    cmd: helper::TCUCmdState,
    pf_state: Option<PfState>,
    cont: Option<fn() -> ContResult>,
}

impl_boxitem!(VPE);

static VPES: StaticCell<[Option<NonNull<VPE>>; 64]> = StaticCell::new([None; 64]);

static IDLE: LazyStaticCell<Box<VPE>> = LazyStaticCell::default();
static OUR: LazyStaticCell<Box<VPE>> = LazyStaticCell::default();

static CUR: StaticCell<Option<Box<VPE>>> = StaticCell::new(None);
static RDY: StaticCell<BoxList<VPE>> = StaticCell::new(BoxList::new());
static BLK: StaticCell<BoxList<VPE>> = StaticCell::new(BoxList::new());

static BOOTSTRAP: StaticCell<bool> = StaticCell::new(true);
static PTS: StaticCell<Vec<Phys>> = StaticCell::new(Vec::new());

pub fn init() {
    extern "C" {
        static _bss_end: usize;
    }

    let (frame, root_pt) = if pex_env().pe_desc.has_virtmem() {
        let (mem_pe, mem_base, mem_size, _) = tcu::TCU::unpack_mem_ep(0).unwrap();

        let base = GlobAddr::new_with(mem_pe, mem_base);

        // use the memory behind ourself for page tables
        let bss_end = unsafe { &_bss_end as *const _ as usize };
        let first_pt = bss_end / cfg::PAGE_SIZE;
        let first_pt = 1 + first_pt as Phys - (cfg::MEM_OFFSET / cfg::PAGE_SIZE) as Phys;
        // we don't need that many PTs here; 512 are enough for now
        let pt_count = cmp::min(first_pt + 512, (mem_size as usize / cfg::PAGE_SIZE) as Phys);
        PTS.get_mut().reserve((pt_count - first_pt) as usize);
        for i in first_pt..pt_count {
            PTS.get_mut()
                .push(cfg::MEM_OFFSET as Phys + i * cfg::PAGE_SIZE as Phys);
        }

        let mut allocator = PTAllocator {
            vpe: kif::pemux::VPE_ID,
        };
        let frame = allocator.allocate_pt().unwrap();
        (frame, Some(base + (frame - cfg::MEM_OFFSET as Phys)))
    }
    else {
        (0, None)
    };

    IDLE.set(Box::new(VPE::new(kif::pemux::IDLE_ID, 0, root_pt)));
    OUR.set(Box::new(VPE::new(kif::pemux::VPE_ID, 0, root_pt)));

    if pex_env().pe_desc.has_virtmem() {
        our().frames.push(frame);
        our().init();
        our().switch_to();
        paging::enable_paging();
    }

    BOOTSTRAP.set(false);
}

pub fn add(id: Id, eps_start: tcu::EpId) -> Result<(), Error> {
    log!(crate::LOG_VPES, "Created VPE {}", id);

    let (frame, root_pt) = if pex_env().pe_desc.has_virtmem() {
        let (mem_pe, mem_base, _, _) = tcu::TCU::unpack_mem_ep(0).unwrap();
        let base = GlobAddr::new_with(mem_pe, mem_base);

        let frame = PTAllocator { vpe: id }.allocate_pt()?;
        (frame, Some(base + (frame - cfg::MEM_OFFSET as Phys)))
    }
    else {
        (0, None)
    };

    let mut vpe = Box::new(VPE::new(id, eps_start, root_pt));

    if pex_env().pe_desc.has_virtmem() {
        vpe.frames.push(frame);
        vpe.init();
    }

    unsafe {
        VPES.get_mut()[id as usize] = Some(NonNull::new_unchecked(vpe.as_mut()));
    }

    make_blocked(vpe);
    Ok(())
}

pub fn get_mut(id: Id) -> Option<&'static mut VPE> {
    if id == kif::pemux::VPE_ID {
        Some(our())
    }
    else {
        VPES.get_mut()[id as usize]
            .as_mut()
            .map(|v| unsafe { v.as_mut() })
    }
}

pub fn our() -> &'static mut VPE {
    OUR.get_mut()
}

pub fn idle() -> &'static mut VPE {
    IDLE.get_mut()
}

#[allow(clippy::borrowed_box)]
pub fn try_cur() -> Option<&'static mut Box<VPE>> {
    CUR.get_mut().as_mut()
}

pub fn cur() -> &'static mut VPE {
    try_cur().unwrap()
}

pub fn has_ready() -> bool {
    !RDY.is_empty()
}

pub fn schedule(mut action: ScheduleAction) -> usize {
    let res = loop {
        let new_state = do_schedule(action);

        let vpe = cur();
        if let Some(new_act) = vpe.exec_cont() {
            action = new_act;
            continue;
        }

        // reset wait_ep here, now that we really run that VPE
        vpe.wait_ep = None;

        break new_state;
    };

    // tell the application whether there are other VPEs ready. if not, it can sleep via TCU without
    // telling us.
    crate::app_env().shared = has_ready() as u64;

    // disable FPU to raise an exception if the app tries to use FPU instructions
    arch::disable_fpu();

    // reprogram timer to consider budget_left of current VPE
    timer::reprogram();

    res
}

fn do_schedule(mut action: ScheduleAction) -> usize {
    let now = tcu::TCU::nanotime();
    let mut next = RDY
        .get_mut()
        .pop_front()
        .unwrap_or_else(|| unsafe { Box::from_raw(idle()) });

    if let Some(old) = try_cur() {
        // reduce budget now in case we decide not to switch below
        old.budget_left = old.budget_left.saturating_sub(now - old.scheduled);

        // save TCU command registers; do that first while still running with that VPE
        old.cmd.save();

        // now change VPE
        let old_id = tcu::TCU::xchg_vpe(next.vpe_reg());

        // are there messages left we care about?
        if action == ScheduleAction::Block && !old.can_block((old_id >> 16) as u16) {
            // if the VPE has budget left, continue with it
            if old.budget_left > 0 {
                let next_id = tcu::TCU::xchg_vpe(old_id);
                next.set_vpe_reg(next_id);
                if next.id() != kif::pemux::IDLE_ID {
                    make_ready(next);
                }
                else {
                    Box::into_raw(next);
                }
                old.scheduled = now;
                return old.user_state_addr;
            }
            // otherwise, preempt it
            else {
                action = ScheduleAction::Preempt;
            }
        }

        old.set_vpe_reg(old_id);
    }
    else {
        tcu::TCU::xchg_vpe(next.vpe_reg());
    }

    // change address space
    next.switch_to();

    // set SP for the next entry
    let new_state = next.user_state_addr;
    isr::set_entry_sp(new_state + size_of::<arch::State>());
    let next_id = next.id();
    next.state = VPEState::Running;

    next.scheduled = now;
    // budget is immediately refilled but we prefer other VPEs while a budget is 0 (see make_ready)
    if next.budget_left == 0 {
        next.budget_left = next.budget_total;
    }
    let next_budget = next.budget_left;

    // restore TCU command registers
    next.cmd.restore();

    // exchange CUR
    if let Some(mut old) = CUR.set(Some(next)) {
        log!(
            crate::LOG_VPES,
            "Switching from {} (budget {}) to {} (budget {}): {:?} old VPE",
            old.id(),
            old.budget_left,
            next_id,
            next_budget,
            action
        );

        if old.id() != kif::pemux::IDLE_ID {
            // block, preempt or kill VPE
            match action {
                ScheduleAction::Block => {
                    make_blocked(old);
                },
                ScheduleAction::Preempt | ScheduleAction::Yield => {
                    make_ready(old);
                },
                ScheduleAction::Kill => {
                    VPES.get_mut()[old.id() as usize] = None;
                },
            }
        }
        else {
            old.state = VPEState::Blocked;
            // don't drop the idle VPE
            Box::into_raw(old);
        }
    }
    else {
        log!(
            crate::LOG_VPES,
            "Switching to {} (budget {})",
            next_id,
            next_budget
        );
    }

    new_state
}

fn make_blocked(mut vpe: Box<VPE>) {
    vpe.state = VPEState::Blocked;
    BLK.get_mut().push_back(vpe);
}

fn make_ready(mut vpe: Box<VPE>) {
    vpe.state = VPEState::Ready;
    // prefer VPEs with budget
    if vpe.budget_left > 0 {
        RDY.get_mut().push_front(vpe);
    }
    else {
        RDY.get_mut().push_back(vpe);
    }
}

pub fn remove_cur(status: u32) {
    remove(cur().id(), status, true, true);
}

pub fn remove(id: Id, status: u32, notify: bool, sched: bool) {
    if let Some(v) = VPES.get_mut()[id as usize].take() {
        let old = match unsafe { &v.as_ref().state } {
            VPEState::Running => CUR.set(None).unwrap(),
            VPEState::Ready => RDY.get_mut().remove_if(|v| v.id() == id).unwrap(),
            VPEState::Blocked => BLK.get_mut().remove_if(|v| v.id() == id).unwrap(),
        };

        log!(
            crate::LOG_VPES,
            "Removed VPE {} with status {}",
            old.id(),
            status
        );

        if notify {
            // change to our VPE (no need to save old vpe_reg; VPE is dead)
            let pex_is_running = (tcu::TCU::get_cur_vpe() & 0xFFFF) == kif::pemux::VPE_ID;
            if !pex_is_running {
                tcu::TCU::xchg_vpe(our().vpe_reg());
            }

            crate::msgbuf().set(kif::pemux::Exit {
                op: kif::pemux::Calls::EXIT.val as u64,
                vpe_sel: old.id(),
                code: status as u64,
            });

            tcu::TCU::send(tcu::KPEX_SEP, crate::msgbuf(), 0, tcu::KPEX_REP).unwrap();

            // switch back to old VPE
            if !pex_is_running {
                let our_vpe = tcu::TCU::xchg_vpe(old.vpe_reg());
                our().set_vpe_reg(our_vpe);
            }
        }

        if sched && old.state == VPEState::Running {
            crate::reg_scheduling(ScheduleAction::Kill);
        }
    }
}

impl VPE {
    pub fn new(id: Id, eps_start: tcu::EpId, root_pt: Option<GlobAddr>) -> Self {
        let aspace = root_pt.map(|r| paging::AddrSpace::new(id, r, PTAllocator { vpe: id }));
        VPE {
            prev: None,
            next: None,
            aspace,
            frames: Vec::new(),
            vpe_reg: id,
            state: VPEState::Blocked,
            #[cfg(any(target_arch = "riscv64", target_arch = "x86_64"))]
            fpu_state: arch::FPUState::default(),
            user_state: arch::State::default(),
            user_state_addr: 0,
            sleeping: false,
            budget_total: TIME_SLICE,
            budget_left: TIME_SLICE,
            scheduled: 0,
            wait_ep: None,
            eps_start,
            cmd: helper::TCUCmdState::new(),
            pf_state: None,
            cont: None,
        }
    }

    pub fn map(
        &mut self,
        virt: usize,
        global: GlobAddr,
        pages: usize,
        perm: kif::PageFlags,
    ) -> Result<(), Error> {
        self.aspace
            .as_mut()
            .unwrap()
            .map_pages(virt, global, pages, perm)
    }

    pub fn translate(&self, virt: usize, perm: kif::PageFlags) -> kif::PTE {
        self.aspace.as_ref().unwrap().translate(virt, perm.bits())
    }

    pub fn id(&self) -> Id {
        self.vpe_reg & 0xFFFF
    }

    pub fn vpe_reg(&self) -> tcu::Reg {
        self.vpe_reg
    }

    pub fn set_vpe_reg(&mut self, val: tcu::Reg) {
        self.vpe_reg = val;
    }

    #[cfg(any(target_arch = "riscv64", target_arch = "x86_64"))]
    pub fn fpu_state(&mut self) -> &mut arch::FPUState {
        &mut self.fpu_state
    }

    pub fn eps_start(&self) -> tcu::EpId {
        self.eps_start
    }

    pub fn msgs(&self) -> u16 {
        (self.vpe_reg >> 16) as u16
    }

    pub fn has_msgs(&self) -> bool {
        self.msgs() != 0
    }

    pub fn add_msg(&mut self) {
        self.vpe_reg += 1 << 16;
    }

    pub fn rem_msgs(&mut self, count: u16) {
        assert!(self.msgs() >= count);
        self.vpe_reg -= (count as u64) << 16;
    }

    pub fn budget_left(&self) -> Nanos {
        self.budget_left
    }

    pub fn user_state(&mut self) -> &mut arch::State {
        &mut self.user_state
    }

    fn can_block(&self, msgs: u16) -> bool {
        if let Some(wep) = self.wait_ep {
            !tcu::TCU::has_msgs(wep)
        }
        else {
            msgs == 0
        }
    }

    fn should_unblock(&self, ep: Option<tcu::EpId>) -> bool {
        match (self.wait_ep, ep) {
            (Some(wait_ep), Some(msg_ep)) => wait_ep == msg_ep,
            // always unblock if the VPE either doesn't wait for a message on a specific EP or if
            // it's a "invalidated EP" unblock.
            _ => true,
        }
    }

    pub fn block(
        &mut self,
        cont: Option<fn() -> ContResult>,
        ep: Option<tcu::EpId>,
        sleep: Option<Nanos>,
    ) {
        log!(crate::LOG_VPES, "Block VPE {} for ep={:?}", self.id(), ep);

        self.cont = cont;
        self.wait_ep = ep;
        if let Some(nanos) = sleep {
            timer::add(self.id(), nanos);
            self.sleeping = true;
        }
        if self.state == VPEState::Running {
            crate::reg_scheduling(ScheduleAction::Block);
        }
    }

    pub fn unblock(&mut self, ep: Option<tcu::EpId>, timer: bool) {
        log!(
            crate::LOG_VPES,
            "Trying to unblock VPE {} for ep={:?}",
            self.id(),
            ep
        );

        if self.user_state_addr != 0 && self.should_unblock(ep) {
            if self.state == VPEState::Blocked {
                let mut vpe = BLK.get_mut().remove_if(|v| v.id() == self.id()).unwrap();
                if !timer && vpe.sleeping {
                    timer::remove(vpe.id());
                }
                vpe.sleeping = false;
                make_ready(vpe);
            }
            if self.state != VPEState::Running {
                crate::reg_scheduling(ScheduleAction::Yield);
            }
        }
    }

    pub fn consume_time(&mut self) {
        let now = tcu::TCU::nanotime();
        let duration = now - self.scheduled;
        self.budget_left = self.budget_left.saturating_sub(duration);
        if self.budget_left == 0 && has_ready() {
            crate::reg_scheduling(ScheduleAction::Preempt);
        }
    }

    pub fn start_pf(&mut self, pf_state: PfState) {
        self.pf_state = Some(pf_state);
    }

    pub fn finish_pf(&mut self) -> PfState {
        self.pf_state.take().unwrap()
    }

    pub fn start(&mut self) {
        assert!(self.user_state_addr == 0);
        if self.id() != kif::pemux::IDLE_ID {
            // remember the current PE
            crate::app_env().pe_id = pex_env().pe_id;
            arch::init_state(
                &mut self.user_state,
                crate::app_env().entry as usize,
                crate::app_env().sp as usize,
            );
        }
        self.user_state_addr = &self.user_state as *const _ as usize;
    }

    pub fn switch_to(&self) {
        if let Some(ref aspace) = self.aspace {
            aspace.switch_to();
        }
    }

    fn exec_cont(&mut self) -> Option<ScheduleAction> {
        self.cont.take().and_then(|cont| {
            let result = cont();
            match result {
                // only resume this VPE if it has been initialized
                ContResult::Success if self.user_state_addr != 0 => None,
                // not initialized yet
                ContResult::Success => Some(ScheduleAction::Block),
                // failed, so remove VPE
                ContResult::Failure => {
                    remove(self.id(), 1, true, false);
                    Some(ScheduleAction::Kill)
                },
                // set the continuation again to retry later
                ContResult::Waiting => {
                    self.cont = Some(cont);
                    // we might have got the PF reply after checking for it, so use TryBlock to not
                    // schedule in case we've received a message.
                    Some(ScheduleAction::Block)
                },
            }
        })
    }

    fn init(&mut self) {
        extern "C" {
            static _user_start: u8;
            static _user_end: u8;
            static _text_start: u8;
            static _text_end: u8;
            static _data_start: u8;
            static _data_end: u8;
            static _bss_start: u8;
            static _bss_end: u8;
        }

        let (mem_pe, mem_base, mem_size, _) = tcu::TCU::unpack_mem_ep(0).unwrap();
        let base = GlobAddr::new_with(mem_pe, mem_base);

        // we have to perform the initialization here, because it calls xlate_pt(), so that the VPE
        // needs to be accessible via get_mut().
        self.aspace.as_mut().unwrap().init();

        // map TCU
        let rw = kif::PageFlags::RW;
        self.map(
            tcu::MMIO_ADDR,
            GlobAddr::new(tcu::MMIO_ADDR as goff),
            tcu::MMIO_SIZE / cfg::PAGE_SIZE,
            kif::PageFlags::U | rw,
        )
        .unwrap();
        self.map(
            tcu::MMIO_PRIV_ADDR,
            GlobAddr::new(tcu::MMIO_PRIV_ADDR as goff),
            tcu::MMIO_PRIV_SIZE / cfg::PAGE_SIZE,
            rw,
        )
        .unwrap();

        // map text, data, and bss
        let rx = kif::PageFlags::RX;
        unsafe {
            self.map_segment(base, &_text_start, &_text_end, rx);
            self.map_segment(base, &_data_start, &_data_end, rw);
            self.map_segment(base, &_bss_start, &_bss_end, rw);
        }

        // map own receive buffer
        let own_rbuf = base + (cfg::PEMUX_RBUF_SPACE - cfg::MEM_OFFSET) as goff;
        assert!(cfg::PEMUX_RBUF_SIZE == cfg::PAGE_SIZE);
        self.map(cfg::PEMUX_RBUF_SPACE, own_rbuf, 1, kif::PageFlags::R)
            .unwrap();

        if self.id() == kif::pemux::VPE_ID {
            // map sleep function for user
            unsafe {
                self.map_segment(base, &_user_start, &_user_end, rx | kif::PageFlags::U);
            }
        }
        else {
            // map application receive buffer
            let perm = kif::PageFlags::R | kif::PageFlags::U;
            self.map_new_mem(base, cfg::RBUF_STD_ADDR, cfg::RBUF_STD_SIZE, perm);
        }

        // map runtime environment
        self.map_new_mem(
            base,
            cfg::ENV_START,
            cfg::ENV_SIZE,
            kif::PageFlags::RW | kif::PageFlags::U,
        );

        // map PTs
        self.map(
            cfg::PE_MEM_BASE,
            base,
            mem_size as usize / cfg::PAGE_SIZE,
            kif::PageFlags::RW,
        )
        .unwrap();

        // map vectors
        #[cfg(target_arch = "arm")]
        self.map(0, base, 1, rx).unwrap();

        // insert fixed entry for messages into TLB
        let virt = crate::msgbuf().bytes().as_ptr() as usize;
        let pte = self.translate(virt, kif::PageFlags::R);
        let phys = pte & !(cfg::PAGE_MASK as u64);
        let mut flags = kif::PageFlags::from_bits_truncate(pte & cfg::PAGE_MASK as u64);
        flags |= kif::PageFlags::FIXED;
        tcu::TCU::insert_tlb(self.id() as u16, virt, phys, flags);
    }

    fn map_new_mem(&mut self, base: GlobAddr, addr: usize, size: usize, perm: kif::PageFlags) {
        for i in 0..(size / cfg::PAGE_SIZE) {
            let frame = self
                .aspace
                .as_mut()
                .unwrap()
                .allocator_mut()
                .allocate_pt()
                .unwrap();

            self.frames.push(frame);
            self.map(
                addr + i * cfg::PAGE_SIZE,
                base + (frame - cfg::MEM_OFFSET as Phys),
                1,
                perm,
            )
            .unwrap();
        }
    }

    fn map_segment(
        &mut self,
        base: GlobAddr,
        start: *const u8,
        end: *const u8,
        perm: kif::PageFlags,
    ) {
        let start = math::round_dn(start as usize, cfg::PAGE_SIZE);
        let end = math::round_up(end as usize, cfg::PAGE_SIZE);
        let pages = (end - start) / cfg::PAGE_SIZE;
        let glob = base + (start - cfg::MEM_OFFSET) as Phys;
        self.map(start, glob, pages, perm).unwrap();
    }
}

impl Drop for VPE {
    fn drop(&mut self) {
        log!(crate::LOG_VPES, "Destroyed VPE {}", self.id());

        // flush+invalidate caches to ensure that we have a fresh view on memory. this is required
        // because of the way the pager handles copy-on-write: it reads the current copy from the
        // owner and updates the version in DRAM. for that reason, the cache for new VPEs needs to
        // be clear, so that the cache loads the current version from DRAM.
        tcu::TCU::flush_cache();

        if let Some(ref mut aspace) = self.aspace {
            // free frames we allocated for env, receive buffers etc.
            for f in &self.frames {
                aspace.allocator_mut().free_pt(*f as paging::MMUPTE);
            }
        }

        // remove VPE from other modules
        if self.sleeping {
            timer::remove(self.id());
        }
        arch::forget_fpu(self.id());
    }
}
