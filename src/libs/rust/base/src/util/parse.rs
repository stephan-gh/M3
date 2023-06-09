/*
 * Copyright (C) 2021-2022 Nils Asmussen, Barkhausen Institut
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

//! Contains utility functions for parsing data types from text

use crate::errors::{Code, Error};
use crate::kif;
use crate::mem::GlobOff;
use crate::time::TimeDuration;

/// Parses an address from the given string
///
/// If the string starts with "0x", the remainder is interpreted hexadecimal, otherwise decimal.
pub fn addr(s: &str) -> Result<GlobOff, Error> {
    if let Some(hex) = s.strip_prefix("0x") {
        GlobOff::from_str_radix(hex, 16)
    }
    else {
        s.parse::<GlobOff>()
    }
    .map_err(|_| Error::new(Code::InvArgs))
}

/// Parses a size from the given string
///
/// The binary prefixes k/K, m/M, and g/G can be used to denote kibibytes, mebibytes, and gibibytes,
/// respectively.
pub fn size(s: &str) -> Result<usize, Error> {
    let mul = match s.chars().last() {
        Some(c) if c >= '0' && c <= '9' => 1,
        Some('k') | Some('K') => 1024,
        Some('m') | Some('M') => 1024 * 1024,
        Some('g') | Some('G') => 1024 * 1024 * 1024,
        _ => return Err(Error::new(Code::InvArgs)),
    };
    Ok(match mul {
        1 => int(s)? as usize,
        m => m * int(&s[0..s.len() - 1])? as usize,
    })
}

/// Parses a time from the given string
///
/// The suffixes ns, µs, ms, and s can be used to denote nanoseconds, microseconds, milliseconds and
/// seconds.
pub fn time(s: &str) -> Result<TimeDuration, Error> {
    let (width, mul) = if s.ends_with("ns") {
        (2, 1)
    }
    else if s.ends_with("µs") {
        (2, 1_000)
    }
    else if s.ends_with("ms") {
        (2, 1_000_000)
    }
    else if s.ends_with('s') {
        (1, 1_000_000_000)
    }
    else {
        return Err(Error::new(Code::InvArgs));
    };
    Ok(TimeDuration::from_nanos(match mul {
        1 => int(s)?,
        m => m * int(&s[0..s.len() - width])?,
    }))
}

/// Parses a u64 from the given string
pub fn int(s: &str) -> Result<u64, Error> {
    s.parse::<u64>().map_err(|_| Error::new(Code::InvArgs))
}

/// Parses a boolean ("true" or "false") from the given string
pub fn bool(s: &str) -> Result<bool, Error> {
    match s {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Ok(int(s)? == 1),
    }
}

/// Parses permissions from the given string
///
/// Expects arbitrary combinations of the letters 'r', 'w', and 'x' to denote read, write, and
/// execute permission, respectively.
pub fn perm(s: &str) -> Result<kif::Perm, Error> {
    let mut perm = kif::Perm::empty();
    for c in s.chars() {
        match c {
            'r' => perm |= kif::Perm::R,
            'w' => perm |= kif::Perm::W,
            'x' => perm |= kif::Perm::X,
            _ => return Err(Error::new(Code::InvArgs)),
        }
    }
    Ok(perm)
}
