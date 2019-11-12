/*
 * Copyright (C) 2015, René Küttner <rene.kuettner@.tu-dresden.de>
 * Economic rights: Technische Universität Dresden (Germany)
 *
 * This file is part of M3 (Microkernel for Minimalist Manycores).
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
use base::const_assert;
use base::dtu;
use core::ptr;

use helper;
use isr;

struct XlateState {
    in_pf: bool,
    req_count: u32,
    reqs: [dtu::Reg; 4],
    cmd: helper::DTUCmdState,
    // store messages in static data to ensure that we don't pagefault
    pf_msg: [u64; 3],
}

impl XlateState {
    const fn new() -> Self {
        XlateState {
            in_pf: false,
            req_count: 0,
            reqs: [0; 4],
            cmd: helper::DTUCmdState::new(),
            pf_msg: [/* PAGEFAULT */ 0, 0, 0],
        }
    }

    fn handle_pf(&mut self, req: dtu::Reg, virt: u64, perm: u64) -> bool {
        if self.in_pf {
            for r in &mut self.reqs {
                if *r == 0 {
                    *r = req;
                    break;
                }
            }
            self.req_count += 1;
            return false;
        }

        // abort the current command, if there is any
        self.cmd.save();

        // get EPs
        let pf_eps = dtu::DTU::get_pfep();
        let sep = (pf_eps & 0xFF) as dtu::EpId;
        let rep = (pf_eps >> 8) as dtu::EpId;
        if sep >= dtu::EP_COUNT {
            return false;
        }

        self.in_pf = true;

        // allow other translation requests in the meantime
        unsafe { asm!("sti" : : : "memory") };

        // send PF message
        self.pf_msg[1] = virt;
        self.pf_msg[2] = perm;
        let msg = &self.pf_msg as *const u64 as *const u8;
        if let Err(e) = dtu::DTU::send(sep, msg, 3 * 8, 0, rep) {
            panic!("VMA: unable to send PF message: {}", e);
        }

        // wait for reply
        let res = loop {
            if isr::is_stopped() {
                break false;
            }

            if let Some(reply) = dtu::DTU::fetch_msg(rep) {
                dtu::DTU::mark_read(rep, reply);
                break true;
            }

            dtu::DTU::sleep().ok();
        };

        unsafe { asm!("cli" : : : "memory") };

        self.in_pf = false;
        res
    }

    fn resume_cmd(&mut self) {
        const_assert!(dtu::CmdOpCode::IDLE.val == 0);
        self.cmd.restore();
    }
}

static STATE: StaticCell<XlateState> = StaticCell::new(XlateState::new());

fn to_dtu_pte(pte: u64) -> dtu::PTE {
    let mut res = pte & !cfg::PAGE_MASK as u64;
    // translate physical address to NoC address
    res = (res & !0x0000_FF00_0000_0000) | ((res & 0x0000_FF00_0000_0000) << 16);
    if (pte & 0x1) != 0 {
        res |= dtu::PTEFlags::R.bits();
    }
    if (pte & 0x2) != 0 {
        res |= dtu::PTEFlags::W.bits();
    }
    if (pte & 0x4) != 0 {
        res |= dtu::PTEFlags::I.bits();
    }
    if (pte & 0x80) != 0 {
        res |= dtu::PTEFlags::LARGE.bits();
    }
    res
}

fn get_pte_at(mut virt: u64, level: u32) -> u64 {
    #[allow(clippy::erasing_op)]
    #[rustfmt::skip]
    const REC_MASK: u64 = ((cfg::PTE_REC_IDX << (cfg::PAGE_BITS + cfg::LEVEL_BITS * 3))
                         | (cfg::PTE_REC_IDX << (cfg::PAGE_BITS + cfg::LEVEL_BITS * 2))
                         | (cfg::PTE_REC_IDX << (cfg::PAGE_BITS + cfg::LEVEL_BITS * 1))
                         | (cfg::PTE_REC_IDX << (cfg::PAGE_BITS + cfg::LEVEL_BITS * 0))) as u64;

    // at first, just shift it accordingly.
    virt >>= cfg::PAGE_BITS + level as usize * cfg::LEVEL_BITS;
    virt <<= cfg::PTE_BITS;

    // now put in one PTE_REC_IDX's for each loop that we need to take
    let shift = (level + 1) as usize;
    let rem_mask = (1 << (cfg::PAGE_BITS + cfg::LEVEL_BITS * (cfg::LEVEL_CNT - shift))) - 1;
    virt |= REC_MASK & !rem_mask;

    // finally, make sure that we stay within the bounds for virtual addresses
    // this is because of recMask, that might actually have too many of those.
    virt &= (1 << (cfg::LEVEL_CNT * cfg::LEVEL_BITS + cfg::PAGE_BITS)) - 1;

    unsafe { *(virt as *const u64) }
}

fn get_pte(virt: u64, perm: u64) -> u64 {
    for lvl in (0..4).rev() {
        let pte = to_dtu_pte(get_pte_at(virt, lvl));
        if lvl == 0 || (!(pte & 0xF) & perm) != 0 || (pte & dtu::PTEFlags::LARGE.bits()) != 0 {
            return pte;
        }
    }
    unreachable!();
}

fn translate_addr(req: dtu::Reg) -> bool {
    let virt = req & !cfg::PAGE_MASK as u64;
    let perm = req & 0xF;
    let xfer_buf = (req >> 5) & 0x7;

    // translate to physical
    let mut pte = if (virt & 0xFFFF_FFFF_F000) == 0x0804_0201_0000 {
        // special case for root pt
        let mut pte: dtu::PTE;
        unsafe { asm!("mov %cr3, $0" : "=r"(pte)) };
        to_dtu_pte(pte | 0x3)
    }
    else if (virt & 0xFFF0_0000_0000) == 0x0800_0000_0000 {
        // in the PTE area, we can assume that all upper level PTEs are present
        to_dtu_pte(get_pte_at(virt, 0))
    }
    else {
        // otherwise, walk through all levels
        get_pte(virt, perm)
    };

    let mut pf = false;
    if (!(pte & 0xF) & perm) != 0 {
        // the first xfer buffer can't raise pagefaults
        if xfer_buf == 0 {
            // the xlate response has to be non-zero, but have no permission bits set
            pte = cfg::PAGE_SIZE as u64;
        }
        else {
            if !STATE.get_mut().handle_pf(req, virt, perm) {
                return false;
            }

            // read PTE again
            pte = to_dtu_pte(get_pte_at(virt, 0));
            pf = true;
        }
    }

    // tell DTU the result; but only if the command has not been aborted or the aborted command
    // did not trigger the translation (in this case, the translation is already aborted, too).
    // TODO that means that aborted commands cause another TLB miss in the DTU, which can then
    // (hopefully) be handled with a simple PT walk. we could improve that by setting the TLB entry
    // right away without continuing the transfer (because that's aborted)
    if !pf || !STATE.cmd.has_cmd() || STATE.cmd.xfer_buf() != xfer_buf {
        dtu::DTU::set_xlate_resp(pte | (xfer_buf << 5));
    }

    if pf {
        STATE.get_mut().resume_cmd();
    }

    pf
}

pub fn handle_xlate(mut xlate_req: dtu::Reg) {
    // acknowledge the translation
    dtu::DTU::set_xlate_req(0);

    if translate_addr(xlate_req) {
        // handle other requests that pagefaulted in the meantime
        while unsafe { ptr::read_volatile(&STATE.req_count) } > 0 {
            for r in &mut STATE.get_mut().reqs {
                xlate_req = *r;
                if xlate_req != 0 {
                    STATE.get_mut().req_count -= 1;
                    *r = 0;
                    translate_addr(xlate_req);
                }
            }
        }
    }
}

pub fn handle_mmu_pf(state: &mut isr::State) {
    let cr2: u64;
    unsafe {
        asm!( "mov %cr2, $0" : "=r"(cr2));
    }

    // PEMux isn't causing PFs
    assert!(state.came_from_user());

    // if we don't use the MMU, we shouldn't get here
    // TODO assert!(env().pedesc.has_mmu());

    let perm = to_dtu_pte((state.error & 0x7) as u64);
    if !STATE.get_mut().handle_pf(0, cr2, perm) {
        if isr::is_stopped() {
            return;
        }

        // if we can't handle the PF, there is something wrong
        panic!("PEMux: pagefault for {:#x} at {:#x}", cr2, { state.rip });
    }

    STATE.get_mut().resume_cmd();
}

pub fn flush_tlb(virt: usize) {
    unsafe { asm!("invlpg ($0)" : : "r"(virt)) }
}
