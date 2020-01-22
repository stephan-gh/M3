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
use base::dtu;
use base::errors::Error;
use base::goff;
use base::kif;
use base::math;

extern "C" fn frame_allocator(vpe: u64) -> paging::MMUPTE {
    get_mut(vpe).unwrap().alloc_frame()
}

extern "C" fn xlate_pt(vpe: u64, phys: paging::MMUPTE) -> usize {
    let vpe = get_mut(vpe).unwrap();
    assert!(phys >= vpe.pts_start() && phys < vpe.pts_end());
    let off = phys - vpe.pts_start();
    if vpe.id() == kif::pemux::VPE_ID {
        off as usize
    }
    else {
        cfg::PE_MEM_BASE + off as usize
    }
}

struct Info {
    pe_desc: kif::PEDesc,
    mem_start: u64,
    mem_end: u64,
}

pub struct VPE {
    aspace: paging::AddrSpace,
    vpe_reg: dtu::Reg,
    pts_start: paging::MMUPTE,
    pts_count: usize,
    pts_pos: usize,
}

static CUR: StaticCell<Option<VPE>> = StaticCell::new(None);
static IDLE: StaticCell<Option<VPE>> = StaticCell::new(None);
static OUR: StaticCell<Option<VPE>> = StaticCell::new(None);
static INFO: StaticCell<Info> = StaticCell::new(Info {
    pe_desc: kif::PEDesc::new_from(0),
    mem_start: 0,
    mem_end: 0,
});

pub fn init(pe_desc: kif::PEDesc, mem_start: u64, mem_size: u64) {
    INFO.get_mut().pe_desc = pe_desc;
    INFO.get_mut().mem_start = mem_start;
    INFO.get_mut().mem_end = mem_start + mem_size;

    let root_pt = mem_start;
    let pts_count = mem_size as usize / cfg::PAGE_SIZE;
    IDLE.set(Some(VPE::new(kif::pemux::IDLE_ID, root_pt, 0, 0)));
    OUR.set(Some(VPE::new(
        kif::pemux::VPE_ID,
        root_pt,
        mem_start,
        pts_count,
    )));

    if pe_desc.has_virtmem() {
        our().init();
        our().switch_to();
    }
}

pub fn add(id: u64) {
    assert!((*CUR).is_none());

    log!(crate::LOG_VPES, "Created VPE {}", id);

    // TODO temporary
    let pt_begin = INFO.get().mem_start + (INFO.get().mem_end - INFO.get().mem_start) / 2;
    let root_pt = pt_begin;
    let pts_count = (INFO.get().mem_end - INFO.get().mem_start) as usize / cfg::PAGE_SIZE;
    CUR.set(Some(VPE::new(id, root_pt, INFO.get().mem_start, pts_count)));

    let vpe = get_mut(id).unwrap();
    if INFO.get().pe_desc.has_virtmem() {
        vpe.init();
        vpe.switch_to();
    }
}

pub fn get_mut(id: u64) -> Option<&'static mut VPE> {
    if id == kif::pemux::VPE_ID {
        return Some(our());
    }
    else {
        let c = cur();
        if c.id() == id {
            return Some(c);
        }
    }
    None
}

pub fn our() -> &'static mut VPE {
    OUR.get_mut().as_mut().unwrap()
}

pub fn cur() -> &'static mut VPE {
    match CUR.get_mut() {
        Some(v) => v,
        None => IDLE.get_mut().as_mut().unwrap(),
    }
}

pub fn remove() {
    if (*CUR).is_some() {
        let old = CUR.set(None).unwrap();
        log!(crate::LOG_VPES, "Destroyed VPE {}", old.id());

        if INFO.get().pe_desc.has_virtmem() {
            // switch back to our own address space
            our().switch_to();
        }
    }
}

impl VPE {
    pub fn new(id: u64, root_pt: goff, pts_start: goff, pts_count: usize) -> Self {
        VPE {
            aspace: paging::AddrSpace::new(id, root_pt, xlate_pt, frame_allocator),
            vpe_reg: id << 19,
            pts_start: paging::noc_to_phys(pts_start) as paging::MMUPTE,
            pts_count,
            // + 1 to skip the root PT
            pts_pos: (root_pt - pts_start) as usize / cfg::PAGE_SIZE + 1,
        }
    }

    pub fn map(
        &self,
        virt: usize,
        phys: goff,
        pages: usize,
        perm: kif::PageFlags,
    ) -> Result<(), Error> {
        self.aspace.map_pages(virt, phys, pages, perm)
    }

    pub fn id(&self) -> u64 {
        self.aspace.id()
    }

    pub fn vpe_reg(&self) -> dtu::Reg {
        self.vpe_reg
    }

    pub fn set_vpe_reg(&mut self, val: dtu::Reg) {
        self.vpe_reg = val;
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

    fn pts_start(&self) -> paging::MMUPTE {
        self.pts_start
    }

    fn pts_end(&self) -> paging::MMUPTE {
        self.pts_start + (self.pts_count * cfg::PAGE_SIZE) as paging::MMUPTE
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

        // map DTU
        let rw = kif::PageFlags::U | kif::PageFlags::RW;
        self.map(
            dtu::MMIO_ADDR,
            dtu::MMIO_ADDR as goff,
            dtu::MMIO_SIZE / cfg::PAGE_SIZE,
            rw,
        )
        .unwrap();
        self.map(
            dtu::MMIO_PRIV_ADDR,
            dtu::MMIO_PRIV_ADDR as goff,
            dtu::MMIO_PRIV_SIZE / cfg::PAGE_SIZE,
            kif::PageFlags::RW,
        )
        .unwrap();

        // map text, data, and bss
        unsafe {
            let rx = kif::PageFlags::U | kif::PageFlags::RX;
            self.map_segment(&_text_start, &_text_end, rx);
            self.map_segment(&_data_start, &_data_end, rw);
            self.map_segment(&_bss_start, &_bss_end, rw);
        }

        // map receive buffers
        // TODO currently the same rbuf space is used for PEMux and apps
        if self.id() == kif::pemux::VPE_ID {
            for i in 0..(cfg::RECVBUF_SIZE / cfg::PAGE_SIZE) {
                let frame = self.alloc_frame();
                assert!(frame != 0);
                self.map(
                    cfg::RECVBUF_SPACE + i * cfg::PAGE_SIZE,
                    paging::phys_to_noc(frame as u64),
                    1,
                    rw,
                )
                .unwrap();
            }
        }
        else {
            let pte = paging::translate(cfg::RECVBUF_SPACE, kif::PageFlags::R.bits());
            self.map(
                cfg::RECVBUF_SPACE,
                pte & !cfg::PAGE_MASK as goff,
                cfg::RECVBUF_SIZE / cfg::PAGE_SIZE,
                rw,
            )
            .unwrap();
        }

        // map PTs
        self.map(
            cfg::PE_MEM_BASE,
            paging::phys_to_noc(self.pts_start as u64),
            self.pts_count,
            kif::PageFlags::RW,
        )
        .unwrap();
    }

    fn switch_to(&self) {
        self.aspace.switch_to();
        dtu::DTU::invalidate_tlb();
    }

    fn map_segment(&self, start: *const u8, end: *const u8, perm: kif::PageFlags) {
        let start = math::round_dn(start as usize, cfg::PAGE_SIZE);
        let end = math::round_up(end as usize, cfg::PAGE_SIZE);
        let pages = (end - start) / cfg::PAGE_SIZE;
        self.map(
            start,
            paging::phys_to_noc((self.pts_start as usize + start) as goff),
            pages,
            perm,
        )
        .unwrap();
    }

    fn alloc_frame(&mut self) -> paging::MMUPTE {
        if self.pts_pos < self.pts_count {
            let res = self.pts_start + (cfg::PAGE_SIZE * self.pts_pos) as paging::MMUPTE;
            self.pts_pos += 1;
            res
        }
        else {
            0
        }
    }
}
