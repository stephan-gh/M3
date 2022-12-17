/*
 * Copyright (C) 2020 Nils Asmussen, Barkhausen Institut
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

mod chan;
mod ctrl;
mod device;

use m3::col::Vec;
use m3::com::MemGate;
use m3::errors::Error;
use m3::session::DiskOperation;

use self::ctrl::DEVICE_COUNT;
use crate::backend::BlockDevice;
use crate::partition::{Partition, PART_COUNT};

#[derive(Clone, Copy)]
pub struct PartDesc {
    chan: u8,
    device: u8,
    part: Partition,
}

pub struct IDEBlockDevice {
    ide_ctrl: ctrl::IDEController,
    devs: [Option<PartDesc>; DEVICE_COUNT * PART_COUNT],
}

impl IDEBlockDevice {
    pub fn new(args: Vec<&str>) -> Result<Self, Error> {
        let mut use_dma = false;
        let mut use_irq = false;
        for s in &args {
            if *s == "-d" {
                use_dma = true;
            }
            else if *s == "-i" {
                use_irq = true;
            }
        }

        let ide_ctrl = ctrl::IDEController::new(use_irq, use_dma)?;

        let mut devs = [None; DEVICE_COUNT * PART_COUNT];
        for c in ide_ctrl.channel() {
            for d in c.devices() {
                for p in d.partitions() {
                    if p.present() {
                        devs[d.id() as usize * PART_COUNT + p.id()] = Some(PartDesc {
                            chan: c.id(),
                            device: d.id(),
                            part: *p,
                        });
                    }
                }
            }
        }

        Ok(IDEBlockDevice { ide_ctrl, devs })
    }
}

impl BlockDevice for IDEBlockDevice {
    fn partition_exists(&self, part: usize) -> bool {
        part < self.devs.len() && self.devs[part].is_some()
    }

    fn read(
        &mut self,
        part: usize,
        buf: &MemGate,
        buf_off: usize,
        disk_off: usize,
        bytes: usize,
    ) -> Result<(), Error> {
        let part_desc = self.devs[part].unwrap();
        self.ide_ctrl.read_write(
            part_desc,
            DiskOperation::READ,
            buf,
            buf_off,
            disk_off,
            bytes,
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
        let part_desc = self.devs[part].unwrap();
        self.ide_ctrl.read_write(
            part_desc,
            DiskOperation::WRITE,
            buf,
            buf_off,
            disk_off,
            bytes,
        )
    }
}
