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

use base::tcu;
use core::intrinsics;

pub struct TCUCmdState {
    cmd_regs: [tcu::Reg; 3],
    xfer_buf: tcu::Reg,
}

impl TCUCmdState {
    pub const fn new() -> Self {
        TCUCmdState {
            cmd_regs: [0; 3],
            xfer_buf: !0,
        }
    }

    pub fn xfer_buf(&self) -> tcu::Reg {
        self.xfer_buf
    }

    pub fn save(&mut self) {
        // abort the current command, if there is any
        let (xfer_buf, old_cmd) = tcu::TCU::abort();
        self.xfer_buf = xfer_buf;

        self.cmd_regs[0] = old_cmd;
        self.cmd_regs[1] = tcu::TCU::read_cmd_reg(tcu::CmdReg::ARG1);
        self.cmd_regs[2] = tcu::TCU::read_cmd_reg(tcu::CmdReg::DATA);
    }

    pub fn restore(&mut self) {
        tcu::TCU::write_cmd_reg(tcu::CmdReg::ARG1, self.cmd_regs[1]);
        tcu::TCU::write_cmd_reg(tcu::CmdReg::DATA, self.cmd_regs[2]);
        if self.cmd_regs[0] != 0 {
            // if there was a command, retry command
            unsafe {
                intrinsics::atomic_fence();
            }
            tcu::TCU::retry(self.cmd_regs[0]);
            self.cmd_regs[0] = 0;
        }
    }
}

pub struct TCUGuard {
    cmd: TCUCmdState,
}

impl TCUGuard {
    pub fn new() -> Self {
        let mut cmd = TCUCmdState::new();
        cmd.save();
        TCUGuard { cmd }
    }

    pub fn state(&self) -> &TCUCmdState {
        &self.cmd
    }
}

impl Drop for TCUGuard {
    fn drop(&mut self) {
        self.cmd.restore();
    }
}
