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

use util;

#[derive(Debug)]
pub struct VarRingBuf {
    size: usize,
    rd_pos: usize,
    wr_pos: usize,
    last: usize,
}

impl VarRingBuf {
    pub fn new(size: usize) -> Self {
        VarRingBuf {
            size,
            rd_pos: 0,
            wr_pos: 0,
            last: size,
        }
    }

    pub fn empty(&self) -> bool {
        self.rd_pos == self.wr_pos
    }

    pub fn size(&self) -> usize {
        self.size
    }

    /// Determines the write position for inserting <size> bytes.
    pub fn get_write_pos(&self, size: usize) -> Option<usize> {
        if self.wr_pos >= self.rd_pos {
            if self.size - self.wr_pos >= size {
                return Some(self.wr_pos);
            }
            else if self.rd_pos > size {
                return Some(0);
            }
        }
        else if self.rd_pos - self.wr_pos > size {
            return Some(self.wr_pos);
        }
        None
    }

    /// Determines the next read position and the amount of bytes available to read. If there is
    /// something to read, the function returns a tuple with the position and the amount. Otherwise,
    /// it returns None.
    pub fn get_read_pos(&self, size: usize) -> Option<(usize, usize)> {
        if self.empty() {
            return None;
        }

        let rpos = if self.rd_pos == self.last {
            0
        }
        else {
            self.rd_pos
        };

        if self.wr_pos > rpos {
            Some((rpos, util::min(self.wr_pos - rpos, size)))
        }
        else {
            Some((rpos, util::min(util::min(self.size, self.last) - rpos, size)))
        }
    }

    /// Advances the write position by `size`.
    ///
    /// The argument `req_size` specifies the number of bytes that have been passed to
    /// get_write_pos. It is used to detect a potential wrap around to zero by get_write_pos, even
    /// if `size` would not require one.
    pub fn push(&mut self, req_size: usize, size: usize) {
        if self.wr_pos >= self.rd_pos {
            if self.size - self.wr_pos >= req_size {
                self.wr_pos += size;
            }
            else if self.rd_pos > req_size && size > 0 {
                self.last = self.wr_pos;
                self.wr_pos = size;
            }
        }
        else if self.rd_pos - self.wr_pos > req_size {
            self.wr_pos += size;
        }
    }

    /// Advances the read position by `size`.
    pub fn pull(&mut self, size: usize) {
        assert!(!self.empty());
        if self.rd_pos == self.last {
            self.rd_pos = 0;
            self.last = self.size;
        }
        self.rd_pos += size;
    }
}
