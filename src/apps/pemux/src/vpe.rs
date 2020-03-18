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
use paging::Allocator;

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
        if INFO.bootstrap {
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
    bootstrap: bool,
}

pub struct VPE {
    prev: Option<NonNull<VPE>>,
    next: Option<NonNull<VPE>>,
    aspace: paging::AddrSpace<PTAllocator>,
    state: State,
    state_addr: usize,
    vpe_reg: tcu::Reg,
    eps_start: tcu::EpId,
}

impl_boxitem!(VPE);

static VPES: StaticCell<[Option<NonNull<VPE>>; 64]> = StaticCell::new([None; 64]);

static IDLE: StaticCell<Option<Box<VPE>>> = StaticCell::new(None);
static OUR: StaticCell<Option<Box<VPE>>> = StaticCell::new(None);

static CUR: StaticCell<Option<Box<VPE>>> = StaticCell::new(None);
static RDY: StaticCell<BoxList<VPE>> = StaticCell::new(BoxList::new());
static BLK: StaticCell<BoxList<VPE>> = StaticCell::new(BoxList::new());

static INFO: StaticCell<Info> = StaticCell::new(Info {
    pe_id: 0,
    pe_desc: kif::PEDesc::new_from(0),
    mem_start: 0,
    mem_end: 0,
    bootstrap: true,
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

    unsafe {
        RDY.get_mut().push_back(Box::from_raw(idle()));
    }

    if pe_desc.has_virtmem() {
        our().init();
        our().switch_to();
        paging::enable_paging();
    }

    INFO.get_mut().bootstrap = false;
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

pub fn have_vpe() -> bool {
    (*CUR).is_some()
}

pub fn our() -> &'static mut VPE {
    OUR.get_mut().as_mut().unwrap()
}

pub fn idle() -> &'static mut VPE {
    IDLE.get_mut().as_mut().unwrap()
}

pub fn cur() -> &'static mut VPE {
    CUR.get_mut().as_mut().unwrap()
}

pub fn schedule(state_addr: usize, block: bool) -> usize {
    let mut next = RDY.get_mut().pop_front().unwrap();

    log!(crate::LOG_VPES, "Switching to VPE {}", next.id());

    // make current
    if INFO.get().pe_desc.has_virtmem() {
        next.switch_to();
    }
    let old_id = tcu::TCU::xchg_vpe(next.vpe_reg());

    // init state, if not already done (now that we've switched to the address space)
    if next.state_addr == 0 {
        if next.id() != kif::pemux::IDLE_ID {
            // remember the current PE
            crate::env().pe_id = INFO.pe_id;
            next.state.init(::env().entry as usize, ::env().sp as usize);
            next.state_addr = &next.state as *const _ as usize;
        }
        else {
            next.state_addr = state_addr;
        }
    }

    let new_state = next.state_addr;
    set_entry_sp(new_state + util::size_of::<State>());

    // switch CUR_VPE reg
    if let Some(mut old) = CUR.set(Some(next)) {
        old.state_addr = state_addr;
        old.set_vpe_reg(old_id);

        // append to ready/blocked list
        let list = if block { BLK.get_mut() } else { RDY.get_mut() };
        log!(
            crate::LOG_VPES,
            "Putting VPE {} to {} list",
            old.id(),
            if block { "blocked" } else { "ready" }
        );
        list.push_back(old);
    }

    new_state
}

pub fn remove(status: u32, notify: bool) {
    if let Some(old) = CUR.set(None) {
        log!(crate::LOG_VPES, "Destroyed VPE {}", old.id());

        VPES.get_mut()[old.id() as usize] = None;

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

        crate::reg_scheduling();
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
            state: State::default(),
            state_addr: 0,
            eps_start,
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

    pub fn start(&mut self) {
        let mut it = BLK.get_mut().iter_mut();
        while let Some(v) = it.next() {
            if v.id() == self.id() {
                let vpe = it.remove().unwrap();
                RDY.get_mut().push_back(vpe);
                crate::reg_scheduling();
                return;
            }
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
    }

    fn switch_to(&self) {
        self.aspace.switch_to();
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
