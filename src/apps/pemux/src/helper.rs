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
use core::intrinsics;

use arch;
use upcalls;

pub struct UpcallsOffGuard {
    prev: bool,
}

impl UpcallsOffGuard {
    pub fn new() -> Self {
        UpcallsOffGuard {
            prev: upcalls::disable(),
        }
    }
}

impl Drop for UpcallsOffGuard {
    fn drop(&mut self) {
        if self.prev {
            upcalls::enable();
        }
    }
}

pub struct IRQsOnGuard {
    prev: bool,
}

impl IRQsOnGuard {
    pub fn new() -> Self {
        IRQsOnGuard {
            prev: arch::enable_ints(),
        }
    }
}

impl Drop for IRQsOnGuard {
    fn drop(&mut self) {
        arch::restore_ints(self.prev);
    }
}

pub struct DTUCmdState {
    cmd_regs: [dtu::Reg; 3],
    xfer_buf: dtu::Reg,
}

impl DTUCmdState {
    pub const fn new() -> Self {
        DTUCmdState {
            cmd_regs: [0; 3],
            xfer_buf: !0,
        }
    }

    #[allow(dead_code)]
    pub fn has_cmd(&self) -> bool {
        self.cmd_regs[0] != 0
    }

    #[allow(dead_code)]
    pub fn xfer_buf(&self) -> dtu::Reg {
        self.xfer_buf
    }

    pub fn save(&mut self) {
        // abort the current command, if there is any
        let (xfer_buf, old_cmd) = dtu::DTU::abort();
        self.xfer_buf = xfer_buf;

        self.cmd_regs[0] = old_cmd;
        self.cmd_regs[1] = dtu::DTU::read_cmd_reg(dtu::CmdReg::ARG1);
        // if a command was being executed, save the DATA register, because we'll overwrite it
        if self.cmd_regs[0] != dtu::CmdOpCode::IDLE.val {
            self.cmd_regs[2] = dtu::DTU::read_cmd_reg(dtu::CmdReg::DATA);
        }
    }

    pub fn restore(&mut self) {
        dtu::DTU::write_cmd_reg(dtu::CmdReg::ARG1, self.cmd_regs[1]);
        if self.cmd_regs[0] != 0 {
            // if there was a command, restore DATA register and retry command
            dtu::DTU::write_cmd_reg(dtu::CmdReg::DATA, self.cmd_regs[2]);
            unsafe {
                intrinsics::atomic_fence();
            }
            dtu::DTU::retry(self.cmd_regs[0]);
            self.cmd_regs[0] = 0;
        }
    }
}

pub struct DTUGuard {
    cmd: DTUCmdState,
}

impl DTUGuard {
    pub fn new() -> Self {
        let mut cmd = DTUCmdState::new();
        cmd.save();
        DTUGuard { cmd }
    }
}

impl Drop for DTUGuard {
    fn drop(&mut self) {
        self.cmd.restore();
    }
}
