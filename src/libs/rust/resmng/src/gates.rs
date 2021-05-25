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

use m3::cell::StaticCell;
use m3::col::{String, Vec};
use m3::com::{RGateArgs, RecvGate};
use m3::errors::Error;
use m3::log;
use m3::math;

pub struct GateManager {
    gates: Vec<(String, RecvGate)>,
}

static MNG: StaticCell<GateManager> = StaticCell::new(GateManager::new());

pub fn get() -> &'static mut GateManager {
    MNG.get_mut()
}

impl GateManager {
    pub const fn new() -> Self {
        Self { gates: Vec::new() }
    }

    pub fn add_rgate(&mut self, name: String, msg_size: usize, slots: usize) -> Result<(), Error> {
        let msg_order = math::next_log2(msg_size);
        let order = msg_order + math::next_log2(slots);
        let rgate = RecvGate::new_with(RGateArgs::default().order(order).msg_order(msg_order))?;

        log!(crate::LOG_GATE, "Created rgate {} @ {}", name, rgate.sel());
        self.gates.push((name, rgate));
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<&RecvGate> {
        for (gname, gate) in &self.gates {
            if gname == name {
                return Some(gate);
            }
        }
        None
    }
}
