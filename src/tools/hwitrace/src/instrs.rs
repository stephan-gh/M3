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

use regex::Regex;

use std::collections::HashMap;
use std::fmt;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Debug)]
pub struct Instruction {
    pub addr: usize,
    pub opcode: u32,
    pub binary: String,
    pub symbol: String,
    pub disasm: String,
}

impl fmt::Display for Instruction {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(
            fmt,
            "Instr {{ addr: {:#x}, opcode: {:#x}, symbol: \x1B[1m{}\x1B[0m {}, disasm: {} }}",
            self.addr, self.opcode, self.binary, self.symbol, self.disasm
        )
    }
}

pub fn parse_instrs<P>(
    cross_prefix: &str,
    instrs: &mut HashMap<usize, Instruction>,
    file: P,
) -> Result<(), Error>
where
    P: AsRef<Path>,
{
    let mut cmd = Command::new(format!("{}objdump", cross_prefix))
        .arg("-dC")
        .arg(file.as_ref().as_os_str())
        .stdout(Stdio::piped())
        .spawn()?;

    let instr_re = Regex::new(r"^\s+([0-9a-f]+):\s+([0-9a-f]+)\s+(.*)").unwrap();

    let binary = file
        .as_ref()
        .file_name()
        .ok_or(Error::InvalPath)?
        .to_str()
        .ok_or(Error::InvalPath)?;
    let stdout = cmd.stdout.as_mut().unwrap();
    let mut reader = BufReader::new(stdout);

    let mut symbol = None;

    let mut line = String::new();
    while reader.read_line(&mut line)? != 0 {
        let tline = line.trim_end();

        // 0000000010000000 <_start>:
        if tline.starts_with(|c: char| c.is_ascii_hexdigit()) && tline.ends_with(">:") {
            let begin = tline.find('<').unwrap();
            let end = tline.rfind('>').unwrap();
            symbol = Some(tline[begin + 1..end].to_string());
        }
        //     10000010:   00a28663                beq     t0,a0,1000001c <_start+0x1c>
        else if let Some(m) = instr_re.captures(tline) {
            let addr = usize::from_str_radix(m.get(1).unwrap().as_str(), 16)?;
            let opcode = u32::from_str_radix(m.get(2).unwrap().as_str(), 16)?;
            let disasm = m.get(3).unwrap().as_str().to_string();

            instrs.insert(addr, Instruction {
                addr,
                opcode,
                binary: binary.to_string(),
                symbol: symbol.clone().ok_or(Error::ObjdumpMalformed)?,
                disasm,
            });
        }

        line.clear();
    }

    match cmd.wait() {
        Ok(status) if !status.success() => Err(Error::ObjdumpFailed(status.code().unwrap())),
        Ok(_) => Ok(()),
        Err(e) => Err(Error::from(e)),
    }
}
