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

#![no_std]
#![feature(const_fn)]
#![feature(core_intrinsics)]

#[macro_use]
extern crate m3;

extern crate thread;

pub mod childs;
pub mod config;
pub mod memory;
mod parser;
pub mod pes;
pub mod sems;
pub mod sendqueue;
pub mod services;

/// Logs general operations
pub const LOG_DEF: bool = true;
/// Logs parsed configs
pub const LOG_CFG: bool = true;
/// Logs child operations
pub const LOG_CHILD: bool = false;
/// Logs semaphore operations
pub const LOG_SEM: bool = false;
/// Logs service operations
pub const LOG_SERV: bool = false;
/// Logs memory operations
pub const LOG_MEM: bool = false;
/// Logs sendqueue operations
pub const LOG_SQUEUE: bool = false;
/// Logs PE operations
pub const LOG_PES: bool = false;
