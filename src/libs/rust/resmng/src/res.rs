/*
 * Copyright (C) 2023 Nils Asmussen, Barkhausen Institut
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

use crate::gates::GateManager;
use crate::memory::MemoryManager;
use crate::mods::ModManager;
use crate::sems::SemManager;
use crate::services::ServiceManager;
use crate::tiles::TileManager;

#[derive(Default)]
pub struct Resources {
    mem: MemoryManager,
    gates: GateManager,
    services: ServiceManager,
    sems: SemManager,
    tiles: TileManager,
    mods: ModManager,
}

impl Resources {
    pub fn memory(&mut self) -> &mut MemoryManager {
        &mut self.mem
    }

    pub fn gates(&mut self) -> &mut GateManager {
        &mut self.gates
    }

    pub fn services(&mut self) -> &mut ServiceManager {
        &mut self.services
    }

    pub fn semaphores(&mut self) -> &mut SemManager {
        &mut self.sems
    }

    pub fn tiles(&mut self) -> &mut TileManager {
        &mut self.tiles
    }

    pub fn mods(&mut self) -> &mut ModManager {
        &mut self.mods
    }
}
