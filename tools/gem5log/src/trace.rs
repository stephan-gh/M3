/*
 * Copyright (C) 2019-2022 Nils Asmussen, Barkhausen Institut
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

use std::collections::BTreeMap;

use std::io::Write;
use std::io::{self, BufRead};

use crate::error::Error;
use crate::symbols;

fn repl_instr_line(
    syms: &BTreeMap<usize, symbols::Symbol>,
    writer: &mut io::StdoutLock<'_>,
    line: &str,
) -> Option<()> {
    // get the first parts:
    // 7802000: C1T00.cpu: T0 : 0x226f3a @ heap_init+26    : mov rcx, DS:[rip + 0x295a7]
    // ^------^ ^--------^ ^^ ^ ^------^ ^---------------------------------------------^
    let mut parts = line.trim_start().splitn(6, ' ');
    let time = parts.next()?;
    let cpu = parts.next()?;
    if !cpu.ends_with(".cpu:") {
        return None;
    }
    let addr = parts.nth(2)?;
    let mut addr_parts = addr.splitn(2, '.');
    let hex_begin = addr_parts.next()?;
    let addr_int = if let Some(hex_num) = hex_begin.strip_prefix("0x") {
        usize::from_str_radix(hex_num, 16).ok()?
    }
    else {
        usize::from_str_radix(hex_begin, 16).ok()?
    };

    // split the rest of the line and omit the symbol and offset:
    let rem = parts.next()?;
    let rem = if rem.starts_with('@') {
        // 7802000: C0T00.cpu: T0 : 0x226f3a @ heap_init+26    : mov rcx, DS:[rip + 0x295a7]
        //                                   ^---------------^   ^-------------------------^
        rem.split_once(" : ").map(|x| x.1)
    }
    else {
        // 7802000: C0T00.cpu: T0 : 0x226f3a     : mov rcx, DS:[rip + 0x295a7]
        //                                         ^-------------------------^
        Some(&rem.trim_start()[2..])
    }?;

    if let Some(sym) = symbols::resolve(syms, addr_int) {
        write!(
            writer,
            "{} {} \x1b[1m{}\x1b[0m @ {:#x} : {}+{:#x} : {}",
            time,
            cpu,
            sym.bin,
            addr_int,
            sym.name,
            addr_int - sym.addr,
            rem
        )
        .ok()?;
    }
    else {
        write!(
            writer,
            "{} {} <Unknown>: {:#x} : {}",
            time, cpu, addr_int, rem
        )
        .ok()?;
    }
    Some(())
}

pub fn generate(syms: &BTreeMap<usize, symbols::Symbol>) -> Result<(), Error> {
    let stdin = io::stdin();
    let mut reader = io::BufReader::new(stdin.lock());

    let stdout = io::stdout();
    let mut writer = stdout.lock();

    let mut line = String::new();
    while reader.read_line(&mut line)? != 0 {
        // try to replace the address with the binary and symbol
        if repl_instr_line(syms, &mut writer, &line).is_none() {
            // if that failed, just write out the line
            writer.write_all(line.as_bytes())?;
        }
        line.clear();
    }
    Ok(())
}
