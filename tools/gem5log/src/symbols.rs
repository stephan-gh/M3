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
use std::fmt;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};

use crate::error::Error;

#[derive(Debug)]
pub struct Symbol {
    pub addr: usize,
    pub size: usize,
    pub name: String,
    pub bin: String,
    pub binoff: usize,
}

impl fmt::Display for Symbol {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(
            fmt,
            "Symbol {{ addr: {:#x}, size: {:#x}, name: {}, bin: {}, binoff: {:#x} }}",
            self.addr, self.size, self.name, self.bin, self.binoff
        )
    }
}

pub fn parse_symbols<P>(syms: &mut BTreeMap<usize, Symbol>, file: P) -> Result<(), Error>
where
    P: AsRef<Path>,
{
    let path = file.as_ref().to_str().ok_or_else(|| Error::InvalPath)?;
    let (path, offset) = if path.contains("+0x") {
        let mut parts = path.split("+0x");
        let path = parts.next().ok_or_else(|| Error::InvalPath)?;
        let offset = parts.next().ok_or_else(|| Error::InvalPath)?;
        let offset = usize::from_str_radix(offset, 16)?;
        (path, offset)
    }
    else {
        (path.as_ref(), 0)
    };

    let mut cmd = Command::new("nm")
        .arg("-SC")
        .arg(path)
        .stdout(Stdio::piped())
        .spawn()?;

    let binary = Path::new(path)
        .file_name()
        .ok_or(Error::InvalPath)?
        .to_str()
        .ok_or(Error::InvalPath)?;
    let stdout = cmd.stdout.as_mut().unwrap();
    let mut reader = BufReader::new(stdout);

    let mut line = String::new();
    while reader.read_line(&mut line)? != 0 {
        // 0021a300 00000030 T kernel::CapTable::act() const
        // ^------^ ^------^ ^ ^---------------------------^
        let parts: Vec<_> = line.trim_end().splitn(4, ' ').collect();
        if parts.len() < 3 {
            continue;
        }

        let parse_line = |parts: Vec<_>| -> Result<(usize, usize, String), Error> {
            let addr = usize::from_str_radix(parts[0], 16)?;
            if parts.len() > 3 {
                Ok((
                    addr,
                    usize::from_str_radix(parts[1], 16)?,
                    parts[3].to_string(),
                ))
            }
            else {
                Err(Error::Internal)
            }
        };

        if let Ok((addr, size, name)) = parse_line(parts) {
            syms.insert(addr + offset, Symbol {
                addr,
                size,
                name,
                bin: binary.to_string(),
                binoff: offset,
            });
        }

        line.clear();
    }

    match cmd.wait() {
        Ok(status) if !status.success() => Err(Error::Nm(status.code().unwrap())),
        Ok(_) => Ok(()),
        Err(e) => Err(Error::from(e)),
    }
}

pub fn resolve(syms: &BTreeMap<usize, Symbol>, addr: usize) -> Option<&Symbol> {
    syms.range(..=addr).nth_back(0).and_then(|(_, sym)| {
        if addr >= sym.binoff + sym.addr && addr < sym.binoff + sym.addr + sym.size {
            Some(sym)
        }
        else {
            None
        }
    })
}
