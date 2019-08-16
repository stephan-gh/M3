use std::collections::BTreeMap;

use std::io::Write;
use std::io::{self};

use crate::error::Error;
use crate::symbols;

fn repl_instr_line(
    syms: &BTreeMap<usize, symbols::Symbol>,
    writer: &mut io::StdoutLock,
    line: &str,
) -> Option<()> {
    // get the first parts:
    // 7802000: pe00.cpu T0 : 0x226f3a @ heap_init+26    : mov rcx, DS:[rip + 0x295a7]
    // ^------^ ^------^ ^^ ^ ^------^ ^---------------------------------------------^
    let mut parts = line.trim_start().splitn(6, ' ');
    let time = parts.next()?;
    let cpu = parts.next()?;
    if !cpu.ends_with(".cpu") {
        return None;
    }
    let addr = parts.nth(2)?;
    let mut addr_parts = addr.splitn(2, '.');
    let addr_int = usize::from_str_radix(&addr_parts.next()?[2..], 16).ok()?;

    // split the rest of the line and omit the symbol and offset:
    let rem = parts.next()?;
    let rem = if rem.starts_with("@") {
        // 7802000: pe00.cpu T0 : 0x226f3a @ heap_init+26    : mov rcx, DS:[rip + 0x295a7]
        //                                 ^---------------^   ^-------------------------^
        rem.splitn(2, " : ").nth(1)
    }
    else {
        // 7802000: pe00.cpu T0 : 0x226f3a     : mov rcx, DS:[rip + 0x295a7]
        //                                       ^-------------------------^
        Some(&rem.trim_start()[2..])
    }?;

    if let Some(sym) = symbols::resolve(syms, addr_int) {
        write!(
            writer,
            "{} {}: \x1b[1m{}\x1b[0m @ {:#x} : {}+{:#x} : {}",
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
            "{} {}: <Unknown>: {:#x} : {}",
            time, cpu, addr_int, rem
        )
        .ok()?;
    }
    Some(())
}

pub fn generate(syms: &BTreeMap<usize, symbols::Symbol>) -> Result<(), Error> {
    crate::with_stdin_lines(syms, |syms, mut writer, line| {
        // try to replace the address with the binary and symbol
        if repl_instr_line(syms, &mut writer, &line).is_none() {
            // if that failed, just write out the line
            writer.write(&line.as_bytes())?;
        }
        Ok(())
    })
}
