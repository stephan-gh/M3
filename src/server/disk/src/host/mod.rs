/*
 * Copyright (C) 2020-2021 Nils Asmussen, Barkhausen Institut
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

use core::cmp;
use m3::cell::StaticRefCell;
use m3::col::Vec;
use m3::com::MemGate;
use m3::errors::{Code, Error};
use m3::goff;
use m3::libc;
use m3::log;
use m3::mem::MaybeUninit;

use crate::backend::BlockDeviceTrait;
use crate::partition::{parse_partitions, Partition};

static TMP_BUF: StaticRefCell<[u8; 4096]> = StaticRefCell::new([0; 4096]);

pub struct BlockDevice {
    disk_fd: i32,
    partitions: Vec<Partition>,
}

impl BlockDevice {
    fn get_file_name(args: Vec<&str>) -> Option<&str> {
        for (i, s) in args.iter().enumerate() {
            if *s == "-f" {
                return Some(args[i + 1]);
            }
        }
        None
    }

    pub fn new(args: Vec<&str>) -> Result<Self, Error> {
        let file_name = Self::get_file_name(args).ok_or_else(|| Error::new(Code::InvArgs))?;

        // open image
        let (disk_fd, parts) = unsafe {
            let disk_fd = libc::open(file_name.as_ptr() as *const libc::c_char, libc::O_RDWR);
            if disk_fd == -1 {
                return Err(Error::new(Code::InvArgs));
            }

            // determine image size
            let mut info: MaybeUninit<libc::stat> = MaybeUninit::uninit().assume_init();
            if libc::fstat(disk_fd, info.as_mut_ptr()) == -1 {
                return Err(Error::new(Code::InvArgs));
            }

            let disk_size = (*info.as_ptr()).st_size;

            log!(crate::LOG_DEF, "Found disk device ({} MiB)", disk_size);

            // read partition table
            let mut buf = TMP_BUF.borrow_mut();
            libc::pread(disk_fd, buf.as_mut() as *mut _ as *mut libc::c_void, 512, 0);

            // parse partitions
            (disk_fd, parse_partitions(&buf[..]))
        };

        for (i, p) in parts.iter().enumerate() {
            if p.present {
                log!(
                    crate::LOG_DEF,
                    "Registered partition {}: {}, {}",
                    i,
                    p.start * 512,
                    p.size * 512
                );
            }
        }

        Ok(Self {
            disk_fd,
            partitions: parts,
        })
    }

    fn access<F>(
        part: &Partition,
        name: &str,
        mut buf_off: usize,
        disk_off: usize,
        mut bytes: usize,
        acc: F,
    ) -> Result<(), Error>
    where
        F: Fn(usize, usize, usize) -> Result<usize, Error>,
    {
        if disk_off.checked_add(bytes).is_none() || disk_off + bytes > part.size as usize * 512 {
            log!(
                crate::LOG_DEF,
                "Invalid request: disk_off={}, bytes={}, part-size: {}",
                disk_off,
                bytes,
                part.size * 512
            );
            return Err(Error::new(Code::InvArgs));
        }

        let mut disk_off = disk_off + part.start as usize * 512;

        log!(
            crate::LOG_DEF,
            "{} {} bytes @ {} in partition {}",
            name,
            bytes,
            disk_off,
            part.id
        );

        while bytes > 0 {
            let amount = acc(disk_off, buf_off, bytes)?;

            disk_off += amount;
            buf_off += amount;
            bytes -= amount;
        }

        Ok(())
    }
}

impl BlockDeviceTrait for BlockDevice {
    fn partition_exists(&self, part: usize) -> bool {
        part < self.partitions.len() && self.partitions[part].present
    }

    fn read(
        &mut self,
        part: usize,
        buf: &MemGate,
        buf_off: usize,
        disk_off: usize,
        bytes: usize,
    ) -> Result<(), Error> {
        let partition = &self.partitions[part];
        Self::access(
            &partition,
            "Reading",
            buf_off,
            disk_off,
            bytes,
            |disk_off, buf_off, bytes| {
                let mut buffer = TMP_BUF.borrow_mut();
                let amount = cmp::min(bytes, buffer.len());
                let res = unsafe {
                    libc::pread(
                        self.disk_fd,
                        buffer.as_mut_ptr() as *mut libc::c_void,
                        amount,
                        disk_off as i64,
                    )
                };
                assert!(res != -1);
                buf.write(&buffer[0..amount], buf_off as goff)?;
                Ok(amount)
            },
        )
    }

    fn write(
        &mut self,
        part: usize,
        buf: &MemGate,
        buf_off: usize,
        disk_off: usize,
        bytes: usize,
    ) -> Result<(), Error> {
        let partition = &self.partitions[part];
        Self::access(
            &partition,
            "Writing",
            buf_off,
            disk_off,
            bytes,
            |disk_off, buf_off, bytes| {
                let mut buffer = TMP_BUF.borrow_mut();
                let amount = cmp::min(bytes, buffer.len());
                buf.read(&mut buffer[0..amount], buf_off as goff)?;
                let res = unsafe {
                    libc::pwrite(
                        self.disk_fd,
                        buffer.as_ptr() as *const libc::c_void,
                        amount,
                        disk_off as i64,
                    )
                };
                assert!(res != -1);
                Ok(amount)
            },
        )
    }
}
