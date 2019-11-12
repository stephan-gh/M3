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

use base::dtu;

use arch::vma;

pub fn handle_ext_req(mut mst_req: dtu::Reg) {
    let cmd = mst_req & 0x3;
    mst_req &= !0x3;

    // ack
    dtu::DTU::set_ext_req(0);

    match From::from(cmd) {
        dtu::ExtReqOpCode::INV_PAGE => vma::flush_tlb(mst_req as usize),
        _ => log!(DEF, "Unexpected cmd: {}", cmd),
    }
}
