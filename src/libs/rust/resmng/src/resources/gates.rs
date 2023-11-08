/*
 * Copyright (C) 2021 Nils Asmussen, Barkhausen Institut
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

use m3::col::{String, Vec};
use m3::com::{RGateArgs, RecvCap};
use m3::errors::Error;
use m3::io::LogFlags;
use m3::log;
use m3::util::math;

#[derive(Default)]
pub struct GateManager {
    gates: Vec<(String, RecvCap)>,
}

impl GateManager {
    pub const fn new() -> Self {
        Self { gates: Vec::new() }
    }

    pub fn add_rgate(&mut self, name: String, msg_size: usize, slots: usize) -> Result<(), Error> {
        let msg_order = math::next_log2(msg_size);
        let order = msg_order + math::next_log2(slots);
        let rgate = RecvCap::new_with(RGateArgs::default().order(order).msg_order(msg_order))?;

        log!(
            LogFlags::ResMngGate,
            "Created rgate {} @ {}",
            name,
            rgate.sel()
        );
        self.gates.push((name, rgate));
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<&RecvCap> {
        self.gates
            .iter()
            .find(|(gname, _gate)| gname == name)
            .map(|(_name, gate)| gate)
    }
}
