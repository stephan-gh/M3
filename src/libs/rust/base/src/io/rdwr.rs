/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2021 Nils Asmussen, Barkhausen Institut
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

//! Contains the read and write traits

use core::cmp;
use core::fmt;
use core::ptr;

use crate::col::{String, Vec};
use crate::errors::{Code, Error};
use crate::util;
use crate::vec;

// this is inspired from std::io::{Read, Write}

/// A trait for objects that support byte-oriented reading
pub trait Read {
    /// Read some bytes from this source into the given buffer and returns the number of read bytes
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error>;

    /// Reads at most `max` bytes into a string and returns it
    fn read_string(&mut self, max: usize) -> Result<String, Error> {
        let mut buf = vec![0u8; max];

        let mut off = 0;
        while off < max {
            let amount = self.read(&mut buf.as_mut_slice()[off..max])?;

            // stop on EOF
            if amount == 0 {
                break;
            }

            off += amount;
        }

        // set final length
        buf.resize(off, 0);
        String::from_utf8(buf).map_err(|_| Error::new(Code::Utf8Error))
    }

    /// Reads all available bytes from this source into the given vector and returns the number of
    /// read bytes
    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> Result<usize, Error> {
        let mut cap = cmp::max(64, buf.capacity() * 2);
        let old_len = buf.len();
        let mut off = old_len;

        'outer: loop {
            buf.resize(cap, 0);

            while off < cap {
                let count = self.read(&mut buf.as_mut_slice()[off..cap])?;

                // stop on EOF
                if count == 0 {
                    break 'outer;
                }

                off += count;
            }

            cap *= 2;
        }

        // set final length
        buf.resize(off, 0);
        Ok(off - old_len)
    }

    /// Reads all available bytes from this source into a string
    fn read_to_string(&mut self) -> Result<String, Error> {
        let mut v = Vec::new();
        self.read_to_end(&mut v)?;
        String::from_utf8(v).map_err(|_| Error::new(Code::Utf8Error))
    }

    /// Reads exactly as many bytes as available in `buf`
    ///
    /// # Errors
    ///
    /// If any I/O error occurs, [`Err`] will be returned. If less bytes are available, [`Err`] will
    /// be returned with [`EndOfFile`](Code::EndOfFile) as the error code.
    fn read_exact(&mut self, mut buf: &mut [u8]) -> Result<(), Error> {
        while !buf.is_empty() {
            match self.read(buf) {
                Err(e) => return Err(e),
                Ok(0) => break,
                Ok(n) => {
                    let tmp = buf;
                    buf = &mut tmp[n..];
                },
            }
        }

        if !buf.is_empty() {
            Err(Error::new(Code::EndOfFile))
        }
        else {
            Ok(())
        }
    }
}

/// A trait for objects that support byte-oriented writing
pub trait Write {
    /// Writes some bytes of the given buffer to this sink and returns the number of written bytes
    fn write(&mut self, buf: &[u8]) -> Result<usize, Error>;

    /// Flushes the underlying buffer, if any
    fn flush(&mut self) -> Result<(), Error> {
        Ok(())
    }

    /// Ensure that the file is made persistent.
    fn sync(&mut self) -> Result<(), Error> {
        Ok(())
    }

    /// Dumps the given array of bytes to this sink
    ///
    /// # Safety
    ///
    /// The address range needs to be readable
    unsafe fn dump_bytes(&mut self, addr: *const u8, len: usize) -> Result<(), Error> {
        let slice = ptr::slice_from_raw_parts(addr, len);
        self.dump_slice(&*slice, addr as usize)
    }

    /// Dumps the given slice to this sink
    fn dump_slice(&mut self, slice: &[u8], addr: usize) -> Result<(), Error> {
        for (i, b) in slice.iter().enumerate() {
            if i % 16 == 0 {
                if i > 0 {
                    self.write(&[b'\n'])?;
                }
                self.write_fmt(format_args!("{:#x}: ", addr + i))?;
            }
            self.write_fmt(format_args!("{:02x} ", b))?;
        }
        if !slice.is_empty() {
            self.write(&[b'\n'])?;
        }
        Ok(())
    }

    /// Writes all bytes of the given buffer to this sink
    ///
    /// # Errors
    ///
    /// If any I/O error occurs, [`Err`] will be returned. If less bytes can be written, [`Err`]
    /// will be returned with [`WriteFailed`](Code::WriteFailed) as the error code.
    fn write_all(&mut self, mut buf: &[u8]) -> Result<(), Error> {
        while !buf.is_empty() {
            match self.write(buf) {
                Err(e) => return Err(e),
                Ok(0) => return Err(Error::new(Code::WriteFailed)),
                Ok(n) => buf = &buf[n..],
            }
        }
        Ok(())
    }

    /// Writes the given formatting arguments into this sink
    fn write_fmt(&mut self, fmt: fmt::Arguments<'_>) -> Result<(), Error> {
        // Create a shim which translates a Write to a fmt::Write and saves
        // off I/O errors. instead of discarding them
        struct Adaptor<'a, T: ?Sized> {
            inner: &'a mut T,
            error: Result<(), Error>,
        }

        impl<'a, T: Write + ?Sized> fmt::Write for Adaptor<'a, T> {
            fn write_str(&mut self, s: &str) -> fmt::Result {
                match self.inner.write_all(s.as_bytes()) {
                    Ok(()) => Ok(()),
                    Err(e) => {
                        self.error = Err(e);
                        Err(fmt::Error)
                    },
                }
            }
        }

        let mut output = Adaptor {
            inner: self,
            error: Ok(()),
        };
        match fmt::write(&mut output, fmt) {
            Ok(()) => Ok(()),
            Err(..) => {
                // check if the error came from the underlying `Write` or not
                if output.error.is_err() {
                    output.error
                }
                else {
                    Err(Error::new(Code::WriteFailed))
                }
            },
        }
    }
}

/// Convenience method that reads `mem::size_of::<T>()` bytes from the given source and interprets
/// them as a `T`
pub fn read_object<T: Default>(r: &mut dyn Read) -> Result<T, Error> {
    let mut obj: T = T::default();
    r.read_exact(util::object_to_bytes_mut(&mut obj))
        .map(|_| obj)
}
