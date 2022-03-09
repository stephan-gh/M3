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
}

impl fmt::Display for Symbol {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(
            fmt,
            "Symbol {{ addr: {:#x}, size: {:#x}, name: {}, bin: {} }}",
            self.addr, self.size, self.name, self.bin
        )
    }
}

pub fn parse_symbols<P>(syms: &mut BTreeMap<usize, Symbol>, file: P) -> Result<(), Error>
where
    P: AsRef<Path>,
{
    let mut cmd = Command::new("nm")
        .arg("-SC")
        .arg(file.as_ref().as_os_str())
        .stdout(Stdio::piped())
        .spawn()?;

    let binary = file
        .as_ref()
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

        let addr = usize::from_str_radix(parts[0], 16)?;
        let (size, name) = if parts[1].len() == 1 {
            (0, parts[2].to_string())
        }
        else if parts.len() > 3 {
            (usize::from_str_radix(parts[1], 16)?, parts[3].to_string())
        }
        else {
            continue;
        };

        if size != 0 {
            syms.insert(addr, Symbol {
                addr,
                size,
                name,
                bin: binary.to_string(),
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
        if addr >= sym.addr && addr < sym.addr + sym.size {
            Some(sym)
        }
        else {
            None
        }
    })
}
