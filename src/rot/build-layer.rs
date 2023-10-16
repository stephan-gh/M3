/*
 * Copyright (C) 2024, Stephan Gerhold <stephan@gerhold.net>
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

use std::env;

fn main() {
    let work_dir = env::current_dir().unwrap();
    println!("cargo:rustc-link-search={}", work_dir.display());
    println!("cargo:rerun-if-changed=memory.ld");
    println!("cargo:rerun-if-changed=../gp.ld");
    println!("cargo:rerun-if-changed=build.rs");
}
