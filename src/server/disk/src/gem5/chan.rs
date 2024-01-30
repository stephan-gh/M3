/*
 * Copyright (C) 2020-2022 Nils Asmussen, Barkhausen Institut
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

use m3::col::Vec;
use m3::com::{opcodes, MemCap, MemGate};
use m3::errors::{Code, Error};
use m3::io::LogFlags;
use m3::kif::Perm;
use m3::log;
use m3::mem::{self, GlobOff};
use m3::rc::Rc;
use m3::tiles::OwnActivity;
use m3::time::TimeDuration;

use super::ctrl::IDE_CTRL_BAR;
use super::device::{ATAReg, BMIReg, CommandStatus, DevOp, Device, PRD};
use super::PartDesc;

pub struct Channel {
    id: u8,
    use_irq: bool,
    use_dma: bool,
    port_base: u16,
    bmr_base: u16,
    pci_dev: Rc<pci::Device>,
    devs: Vec<Device>,
}

impl Channel {
    pub fn new(
        pci_dev: Rc<pci::Device>,
        ide_ctrl: &pci::Info,
        use_irq: bool,
        use_dma: bool,
        id: u8,
        port_base: u16,
    ) -> Result<Self, Error> {
        let mut chan = Self {
            id,
            use_irq,
            use_dma,
            port_base,
            bmr_base: ide_ctrl.bar(IDE_CTRL_BAR).addr() as u16,
            pci_dev,
            devs: Vec::new(),
        };
        log!(
            LogFlags::DiskDbg,
            "chan[{}] initializing with ports={}, bmr={}",
            id,
            port_base,
            chan.bmr_base,
        );

        chan.check_bus()?;

        // init DMA
        if use_dma && chan.bmr_base != 0 {
            chan.bmr_base += id as u16 * 0x8;
            log!(LogFlags::DiskDbg, "chan[{}] using DMA", chan.id);
        }
        else {
            chan.use_dma = false;
        }

        // init attached devices; begin with slave
        for i in (0..2).rev() {
            let did = id * 2 + i;
            match Device::new(did, &chan) {
                Err(e) => log!(
                    LogFlags::DiskChan,
                    "chan[{}] ignoring device {}: {}",
                    id,
                    did,
                    e
                ),
                Ok(d) => {
                    log!(
                        LogFlags::DiskChan,
                        "chan[{}] found device {}: {} MiB",
                        id,
                        did,
                        d.size() / (1024 * 1024)
                    );
                    for p in d.partitions() {
                        log!(LogFlags::DiskChan, "chan[{}] registered {:?}", id, p);
                    }
                    chan.devs.push(d)
                },
            }
        }

        Ok(chan)
    }

    pub fn id(&self) -> u8 {
        self.id
    }

    pub fn use_dma(&self) -> bool {
        self.use_dma
    }

    pub fn use_irq(&self) -> bool {
        self.use_irq
    }

    pub fn devices(&self) -> &Vec<Device> {
        &self.devs
    }

    pub fn read_write(
        &self,
        desc: PartDesc,
        op: opcodes::Disk,
        buf: &MemGate,
        buf_off: usize,
        disk_off: usize,
        bytes: usize,
    ) -> Result<(), Error> {
        let dev = &self.devs[desc.device as usize];

        // check arguments
        let part_size = desc.part.sector_count() as usize * dev.sector_size();
        if disk_off.checked_add(bytes).is_none() || disk_off + bytes > part_size {
            log!(
                LogFlags::DiskChan,
                "Invalid request: disk_off={}, bytes={}, part-size: {}",
                disk_off,
                bytes,
                part_size
            );
            return Err(Error::new(Code::InvArgs));
        }

        let lba = desc.part.start_sector() as u64 + disk_off as u64 / dev.sector_size() as u64;
        let count = bytes / dev.sector_size();

        let dev_op = match op {
            opcodes::Disk::Read => DevOp::READ,
            _ => DevOp::WRITE,
        };

        log!(
            LogFlags::DiskChan,
            "chan[{}] {:?} {} sectors at {}",
            self.id,
            dev_op,
            count,
            lba,
        );

        let dev_buf = buf.derive_cap(
            buf_off as GlobOff,
            (bytes + mem::size_of::<PRD>()) as GlobOff,
            Perm::RW,
        )?;
        self.set_dma_buffer(&dev_buf)?;

        dev.read_write(self, dev_op, buf, buf_off, lba, dev.sector_size(), count)
    }

    pub fn set_dma_buffer(&self, buf: &MemCap) -> Result<(), Error> {
        self.pci_dev.set_dma_buffer(buf)
    }

    pub fn select(&self, id: u8, extra: u8) -> Result<(), Error> {
        log!(
            LogFlags::DiskDbg,
            "chan[{}] selecting device {:x} with {:x}",
            self.id,
            id,
            extra
        );
        self.write_pio(ATAReg::DriveSelect, extra | ((id & 0x1) << 4))
            .map(|_| self.wait())
    }

    pub fn wait(&self) {
        for _ in 0..4 {
            self.pci_dev
                .read_config::<u8>((self.port_base + ATAReg::CmdStatus as u16) as GlobOff)
                .unwrap();
        }
    }

    pub fn wait_irq(&self) -> Result<(), Error> {
        if self.use_irq {
            log!(LogFlags::DiskDbg, "chan[{}] waiting for IRQ...", self.id);
            self.pci_dev.wait_for_irq()
        }
        else {
            Ok(())
        }
    }

    pub fn wait_until(
        &self,
        timeout: TimeDuration,
        sleep: TimeDuration,
        set: CommandStatus,
        unset: CommandStatus,
    ) -> Result<(), Error> {
        log!(
            LogFlags::DiskDbg,
            "chan[{}] waiting for set={:?}, unset={:?}",
            self.id,
            set,
            unset
        );

        let mut elapsed = TimeDuration::ZERO;
        while elapsed < timeout {
            let status: u8 = self.read_pio(ATAReg::CmdStatus)?;
            if (status & CommandStatus::ERROR.bits()) != 0 {
                // TODO convert error code
                self.read_pio(ATAReg::Error)?;
                return Err(Error::new(Code::InvArgs));
            }
            if (status & set.bits()) == set.bits() && (status & unset.bits()) == 0 {
                return Ok(());
            }

            OwnActivity::sleep_for(sleep)?;
            elapsed += sleep;
        }

        Err(Error::new(Code::Timeout))
    }

    pub fn read_pio<T>(&self, reg: ATAReg) -> Result<T, Error> {
        self.pci_dev
            .read_reg((self.port_base + reg as u16) as GlobOff)
    }

    pub fn write_pio<T>(&self, reg: ATAReg, val: T) -> Result<(), Error> {
        self.pci_dev
            .write_reg((self.port_base + reg as u16) as GlobOff, val)
    }

    pub fn read_pio_words(&self, reg: ATAReg, buf: &mut [u16]) -> Result<(), Error> {
        for b in buf.iter_mut() {
            *b = self.read_pio(reg)?;
        }
        Ok(())
    }

    pub fn write_pio_words(&self, reg: ATAReg, buf: &[u16]) -> Result<(), Error> {
        for b in buf.iter() {
            self.write_pio(reg, b)?;
        }
        Ok(())
    }

    pub fn read_bmr<T>(&self, reg: BMIReg) -> Result<T, Error> {
        self.pci_dev
            .read_reg((self.bmr_base + reg as u16) as GlobOff)
    }

    pub fn write_bmr<T>(&self, reg: BMIReg, val: T) -> Result<(), Error> {
        self.pci_dev
            .write_reg((self.bmr_base + reg as u16) as GlobOff, val)
    }

    fn check_bus(&self) -> Result<(), Error> {
        for i in (0..2).rev() {
            // begin with slave. master should respond if there is no slave
            self.write_pio::<u8>(ATAReg::DriveSelect, i << 4)?;
            self.wait();

            // write some arbitrary values to some registers
            self.write_pio(ATAReg::Address1, 0xF1u8)?;
            self.write_pio(ATAReg::Address2, 0xF2u8)?;
            self.write_pio(ATAReg::Address3, 0xF3u8)?;

            // if we can read them back, the bus is present
            // check for value, one must not be floating
            if self.read_pio::<u8>(ATAReg::Address1)? == 0xF1
                && self.read_pio::<u8>(ATAReg::Address2)? == 0xF2
                && self.read_pio::<u8>(ATAReg::Address3)? == 0xF3
            {
                return Ok(());
            }
        }
        Err(Error::new(Code::NotFound))
    }
}
