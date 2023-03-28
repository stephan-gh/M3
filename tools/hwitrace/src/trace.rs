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

use crate::error::Error;
use crate::instrs::Instruction;

use regex::Regex;

use std::collections::HashMap;
use std::io::{self, BufRead};

pub fn enrich_trace(instrs: &HashMap<usize, Instruction>) -> Result<(), Error> {
    let re = Regex::new(r"^\s*\d+: 0x([0-9a-f]+) 0x([0-9a-f]+) \d \d \d 0x[0-9a-f]+ 0x[0-9a-f]+")
        .unwrap();

    let mut last_symbol = String::new();
    let mut last_binary = String::new();

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = line?;
        let line = line.trim_end();

        //   12: 0x10000b24 0x01a93023 1 0 0 0x0000000000000005 0x00000000
        if let Some(m) = re.captures(line) {
            let addr = usize::from_str_radix(m.get(1).unwrap().as_str(), 16)?;
            let _opcode = usize::from_str_radix(m.get(2).unwrap().as_str(), 16)?;

            if let Some(instr) = instrs.get(&addr) {
                if instr.symbol != last_symbol || instr.binary != last_binary {
                    println!("\x1B[1m{}\x1B[0m - {}:", instr.binary, instr.symbol);
                    last_symbol = instr.symbol.clone();
                    last_binary = instr.binary.clone();
                }

                println!("{} {}", line, instr.disasm);
            }
            else {
                println!("{} ??", line);
            }
        }
    }

    Ok(())
}
