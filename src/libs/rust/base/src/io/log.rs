/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
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

//! Contains the logger

use core::cmp;

use crate::cell::{RefMut, StaticCell, StaticRefCell};
use crate::errors::Error;
use crate::io::{Serial, Write};
use crate::tcu::TCU;

/// Default log message type
pub const DEF: bool = true;
/// Logs heap operations
pub const HEAP: bool = false;
/// Logs file system operations
pub const FS: bool = false;
/// Logs server operations
pub const SERV: bool = false;
/// Logs TCU operations
pub const TCU: bool = false;
/// Logs critical TCU errors (only on host)
pub const TCU_ERR: bool = true;
/// Logs networking events
pub const NET: bool = false;
/// Logs global<->phys address translations
pub const TRANSLATE: bool = false;

const MAX_LINE_LEN: usize = 180;
const SUFFIX: &[u8] = b"\x1B[0m";

static LOG_READY: StaticCell<bool> = StaticCell::new(false);
static LOG: StaticRefCell<Log> = StaticRefCell::new(Log::new());

/// A buffered logger that writes to the serial line
pub struct Log {
    serial: Serial,
    buf: [u8; MAX_LINE_LEN],
    pos: usize,
    time_pos: usize,
    start_pos: usize,
}

impl Log {
    /// Returns the logger
    pub fn get() -> Option<RefMut<'static, Log>> {
        match LOG_READY.get() {
            true => Some(LOG.borrow_mut()),
            false => None,
        }
    }

    pub(crate) const fn new() -> Self {
        Log {
            serial: Serial::new(),
            buf: [0; MAX_LINE_LEN],
            pos: 0,
            time_pos: 0,
            start_pos: 0,
        }
    }

    fn write_bytes(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.put_char(*b)
        }
    }

    fn put_char(&mut self, c: u8) {
        self.buf[self.pos] = c;
        self.pos += 1;

        if c == b'\n' || self.pos + SUFFIX.len() + 1 >= MAX_LINE_LEN {
            for c in SUFFIX {
                self.buf[self.pos] = *c;
                self.pos += 1;
            }
            if c != b'\n' {
                self.buf[self.pos] = b'\n';
                self.pos += 1;
            }

            self.flush().unwrap();
        }
    }

    pub(crate) fn init(&mut self, tile_id: u64, name: &str) {
        let colors = ["31", "32", "33", "34", "35", "36"];
        let begin = match name.rfind('/') {
            Some(b) => b + 1,
            None => 0,
        };
        let len = cmp::min(name.len() - begin, 8);

        self.pos = 0;
        self.write_fmt(format_args!(
            "\x1B[0;{}m[T{:X}:{:<8}@",
            colors[(tile_id as usize) % colors.len()],
            tile_id,
            &name[begin..begin + len]
        ))
        .unwrap();
        self.time_pos = self.pos;
        self.start_pos = self.pos + 11 + 2;
        self.pos = self.start_pos;
    }
}

impl Write for Log {
    fn flush(&mut self) -> Result<(), Error> {
        let length = self.pos;
        self.pos = self.time_pos;
        self.write_fmt(format_args!(
            "{:11}] ",
            (TCU::nanotime() / 1000) % 10_000_000_000
        ))
        .unwrap();
        self.serial.write(&self.buf[0..length])?;
        self.pos = self.start_pos;
        Ok(())
    }

    fn sync(&mut self) -> Result<(), Error> {
        Ok(())
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, Error> {
        self.write_bytes(buf);
        Ok(buf.len())
    }
}

/// Initializes the logger
pub fn init(tile_id: u64, name: &str) {
    LOG_READY.set(true);
    Log::get().unwrap().init(tile_id, name);
}
