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

pub mod gates;
pub mod memory;
pub mod mods;
pub mod sems;
pub mod services;
pub mod tiles;

use gates::GateManager;
use memory::MemoryManager;
use mods::ModManager;
use sems::SemManager;
use services::ServiceManager;
use tiles::TileManager;

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
    pub fn memory(&self) -> &MemoryManager {
        &self.mem
    }

    pub fn memory_mut(&mut self) -> &mut MemoryManager {
        &mut self.mem
    }

    pub fn gates(&self) -> &GateManager {
        &self.gates
    }

    pub fn gates_mut(&mut self) -> &mut GateManager {
        &mut self.gates
    }

    pub fn services(&self) -> &ServiceManager {
        &self.services
    }

    pub fn services_mut(&mut self) -> &mut ServiceManager {
        &mut self.services
    }

    pub fn semaphores(&self) -> &SemManager {
        &self.sems
    }

    pub fn semaphores_mut(&mut self) -> &mut SemManager {
        &mut self.sems
    }

    pub fn tiles(&self) -> &TileManager {
        &self.tiles
    }

    pub fn tiles_mut(&mut self) -> &mut TileManager {
        &mut self.tiles
    }

    pub fn mods(&self) -> &ModManager {
        &self.mods
    }

    pub fn mods_mut(&mut self) -> &mut ModManager {
        &mut self.mods
    }
}
