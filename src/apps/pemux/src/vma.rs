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
use base::kif::{PageFlags, PTE};
use core::ptr;
use paging;

use helper;
use upcalls;

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

    fn handle_pf(&mut self, req: dtu::Reg, virt: usize, perm: PageFlags) -> bool {
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

        self.in_pf = true;

        // disable upcalls during DTU::send, because don't want to abort this command
        upcalls::disable();

        // allow other translation requests in the meantime
        let _guard = helper::IRQsOnGuard::new();

        // send PF message
        self.pf_msg[1] = virt as u64;
        self.pf_msg[2] = perm.bits();
        let msg = &self.pf_msg as *const u64 as *const u8;
        if let Err(e) = dtu::DTU::send(dtu::PG_SEP, msg, 3 * 8, 0, dtu::PG_REP) {
            panic!(
                "VMA: unable to send PF message for virt={:#x}, perm={:?}: {}",
                virt, perm, e
            );
        }

        upcalls::enable();

        // wait for reply
        let res = loop {
            if crate::is_stopped() {
                break false;
            }

            if let Some(reply) = dtu::DTU::fetch_msg(dtu::PG_REP) {
                dtu::DTU::ack_msg(dtu::PG_REP, reply);
                break true;
            }

            dtu::DTU::wait_for_msg(dtu::PG_REP, 0).ok();
        };

        self.in_pf = false;
        res
    }

    fn resume_cmd(&mut self) {
        const_assert!(dtu::CmdOpCode::IDLE.val == 0);
        self.cmd.restore();
    }
}

static STATE: StaticCell<XlateState> = StaticCell::new(XlateState::new());

fn translate_addr(req: dtu::Reg) -> bool {
    let virt = req as usize & !cfg::PAGE_MASK as usize;
    let perm = PageFlags::from_bits_truncate((req >> 1) & PageFlags::RW.bits());
    let xfer_buf = (req >> 5) & 0x7;

    // perform page table walk
    let mut pte = paging::translate(virt, perm.bits());

    let mut pf = false;
    if (!(pte & PageFlags::RW.bits()) & perm.bits()) != 0 {
        // the first xfer buffer can't raise pagefaults
        if xfer_buf == 0 {
            // the xlate response has to be non-zero, but have no permission bits set
            pte = cfg::PAGE_SIZE as PTE;
        }
        else {
            if !STATE.get_mut().handle_pf(req, virt, perm) {
                return false;
            }

            // read PTE again
            pte = paging::translate(virt, perm.bits());
            pf = true;
        }
    }

    // tell DTU the result; but only if the command has not been aborted or the aborted command
    // did not trigger the translation (in this case, the translation is already aborted, too).
    // TODO that means that aborted commands cause another TLB miss in the DTU, which can then
    // (hopefully) be handled with a simple PT walk. we could improve that by setting the TLB entry
    // right away without continuing the transfer (because that's aborted)
    if !pf || !STATE.cmd.has_cmd() || STATE.cmd.xfer_buf() != xfer_buf {
        dtu::DTU::set_core_resp(pte | (xfer_buf << 5));
    }

    if pf {
        STATE.get_mut().resume_cmd();
    }

    pf
}

pub fn handle_xlate(mut xlate_req: dtu::Reg) {
    if translate_addr(xlate_req) {
        // handle other requests that pagefaulted in the meantime. use volatile because STATE might
        // have changed after the call to translate_addr through a nested IRQ.
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

pub fn handle_pf(user: bool, virt: usize, perm: PageFlags, ip: usize) {
    // PEMux isn't causing PFs
    assert!(user, "pagefault for {:#x} at {:#x}", virt, ip);

    if !STATE.get_mut().handle_pf(0, virt, perm) {
        if crate::is_stopped() {
            return;
        }

        // if we can't handle the PF, there is something wrong
        panic!("PEMux: pagefault for {:#x} at {:#x}", virt, ip);
    }

    STATE.get_mut().resume_cmd();
}
