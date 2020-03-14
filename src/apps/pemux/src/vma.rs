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
use base::errors::Error;
use base::kif::{DefaultReply, PageFlags, PTE};
use base::tcu;
use base::util;
use core::ptr;

use helper;
use vpe;

struct XlateState {
    in_pf: bool,
    cmd_saved: bool,
    req_count: u32,
    reqs: [tcu::Reg; 4],
    cmd: helper::TCUCmdState,
    // store messages in static data to ensure that we don't pagefault
    pf_msg: [u64; 3],
}

impl XlateState {
    const fn new() -> Self {
        XlateState {
            in_pf: false,
            cmd_saved: false,
            req_count: 0,
            reqs: [0; 4],
            cmd: helper::TCUCmdState::new(),
            pf_msg: [/* PAGEFAULT */ 0, 0, 0],
        }
    }

    fn handle_pf(&mut self, req: tcu::Reg, virt: usize, perm: PageFlags) -> Result<bool, Error> {
        if self.in_pf {
            for r in &mut self.reqs {
                if *r == 0 {
                    *r = req;
                    break;
                }
            }
            self.req_count += 1;
            return Ok(false);
        }

        // abort the current command, if there is any
        self.cmd.save();

        self.cmd_saved = true;
        self.in_pf = true;

        // allow other translation requests in the meantime
        let eps_start = vpe::cur().eps_start();
        let _irqs_on = helper::IRQsOnGuard::new();

        {
            // disable upcalls during TCU::send, because don't want to abort this command
            let _upcalls_off = helper::UpcallsOffGuard::new();

            // send PF message
            self.pf_msg[1] = virt as u64;
            self.pf_msg[2] = perm.bits();
            let msg = &self.pf_msg as *const u64 as *const u8;
            let size = util::size_of_val(&self.pf_msg);
            let res = tcu::TCU::send(
                eps_start + tcu::PG_SEP_OFF,
                msg,
                size,
                0,
                eps_start + tcu::PG_REP_OFF,
            );
            if let Err(e) = res {
                self.in_pf = false;
                return Err(e);
            }
        }

        // wait for reply
        let res = loop {
            if !vpe::have_vpe() {
                break Ok(true);
            }

            if let Some(msg) = tcu::TCU::fetch_msg(eps_start + tcu::PG_REP_OFF) {
                let err = {
                    let reply = msg.get_data::<DefaultReply>();
                    let err = reply.error as u32;
                    tcu::TCU::ack_msg(eps_start + tcu::PG_REP_OFF, msg);
                    err
                };

                if err != 0 {
                    break Err(Error::from(err));
                }
                else {
                    break Ok(true);
                }
            }

            tcu::TCU::wait_for_msg(eps_start + tcu::PG_REP_OFF, 0).ok();
        };

        self.in_pf = false;
        res
    }

    fn resume_cmd(&mut self) {
        if self.cmd_saved {
            self.cmd_saved = false;
            self.cmd.restore();
        }
    }
}

static STATE: StaticCell<XlateState> = StaticCell::new(XlateState::new());

fn translate_addr(req: tcu::Reg) {
    let asid = req >> 48;
    let virt = ((req & 0xFFFF_FFFF_FFFF) as usize) & !cfg::PAGE_MASK as usize;
    let perm = PageFlags::from_bits_truncate((req >> 1) & PageFlags::RW.bits());
    let xfer_buf = (req >> 5) & 0x7;

    // perform page table walk
    let mut pte = vpe::get_mut(asid).map_or(0, |v| v.translate(virt, perm));
    let cmd_saved = STATE.cmd_saved;
    let mut aborted = false;

    if (!(pte & PageFlags::RW.bits()) & perm.bits()) != 0 {
        // the first xfer buffer can't raise pagefaults
        if xfer_buf == 0 {
            // the xlate response has to be non-zero, but have no permission bits set
            pte = cfg::PAGE_SIZE as PTE;
        }
        else {
            aborted = true;
            let pf_handled = STATE.get_mut().handle_pf(req, virt, perm);
            match pf_handled {
                Err(_) => pte = cfg::PAGE_SIZE as PTE, // as above
                // read PTE again
                Ok(true) => pte = vpe::get_mut(asid).map_or(0, |v| v.translate(virt, perm)),
                Ok(false) => return,
            }
        }
    }

    // tell TCU the result; but only if the command has not been aborted or the aborted command
    // did not trigger the translation (in this case, the translation is already aborted, too).
    if !aborted || STATE.cmd.xfer_buf() != xfer_buf {
        tcu::TCU::set_core_resp(pte | (xfer_buf << 5));
    }

    if cmd_saved != STATE.cmd_saved {
        assert!(STATE.cmd_saved);
        STATE.get_mut().resume_cmd();
    }
}

pub fn handle_xlate(mut xlate_req: tcu::Reg) {
    translate_addr(xlate_req);

    if !STATE.in_pf {
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

pub fn handle_pf(
    state: &crate::arch::State,
    virt: usize,
    perm: PageFlags,
    ip: usize,
) -> Result<(), Error> {
    // PEMux isn't causing PFs
    if crate::nesting_level() != 1 {
        // save the current command to ensure that we can use the print command
        let _cmd_saved = helper::TCUGuard::new();
        panic!("pagefault for {:#x} at {:#x}", virt, ip);
    }

    if let Err(e) = STATE.get_mut().handle_pf(0, virt, perm) {
        log!(crate::LOG_ERR, "Pagefault for {:#x} with {:?}", virt, state);
        return Err(e);
    }

    STATE.get_mut().resume_cmd();

    Ok(())
}
