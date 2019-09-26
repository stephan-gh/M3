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
use base::envdata;
use base::io;
use core::intrinsics;
use core::ptr;

use arch::isr;
use arch::vma;
use vpe;

int_enum! {
    pub struct PEMuxCtrl : u64 {
        const NONE    = 0;
        const RESTORE = 1 << 0; // restore operation required
        const WAITING = 1 << 1; // set by the kernel if a signal is required
        const SIGNAL  = 1 << 2; // used to signal completion to the kernel
    }
}

#[used]
#[link_section = ".pemux"]
static PEMUX_FLAGS: StaticCell<[u64; 2]> = StaticCell::new([0, 0]);

fn env() -> &'static mut envdata::EnvData {
    unsafe { intrinsics::transmute(cfg::ENV_START) }
}

fn flags_get() -> u64 {
    unsafe { ptr::read_volatile(&PEMUX_FLAGS[1]) }
}

fn flags_set(val: u64) {
    unsafe { ptr::write_volatile(&mut PEMUX_FLAGS.get_mut()[1], val) }
}

fn signal() {
    unsafe { intrinsics::atomic_fence() };
    // tell the kernel that we are ready
    flags_set(PEMuxCtrl::SIGNAL.val);
}

pub fn handle_pemux(state: &mut isr::State) {
    let flags = flags_get();

    if (flags & PEMuxCtrl::RESTORE.val) != 0 {
        // notify the kernel as early as possible
        signal();

        // remember the current PE (might have changed since last switch)
        let vpe_id = flags >> 48;
        env().pe_id = (flags >> 32) & 0xFFFF;

        // reinit io with correct PE id
        // TODO there should be a better way
        io::init(env().pe_id, "pemux");

        state.init(env().entry as usize, env().sp as usize);

        vpe::add(vpe_id);
        return;
    }

    if (flags & PEMuxCtrl::WAITING.val) != 0 {
        signal();
    }
}

fn handle_stop(state: &mut isr::State) {
    state.stop();
}

pub fn handle_ext_req(state: &mut isr::State, mut mst_req: dtu::Reg) {
    let cmd = mst_req & 0x3;
    mst_req &= !0x3;

    // ack
    dtu::DTU::set_ext_req(0);

    match From::from(cmd) {
        dtu::ExtReqOpCode::INV_PAGE => vma::flush_tlb(mst_req as usize),
        dtu::ExtReqOpCode::PEMUX => handle_pemux(state),
        dtu::ExtReqOpCode::STOP => handle_stop(state),
        _ => log!(DEF, "Unexpected cmd: {}", cmd),
    }

    #[cfg(target_arch = "arm")]
    base::dtu::DTU::clear_irq();
}
