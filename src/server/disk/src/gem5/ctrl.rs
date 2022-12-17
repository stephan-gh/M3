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
use m3::com::MemGate;
use m3::errors::Error;
use m3::int_enum;
use m3::kif;
use m3::log;
use m3::rc::Rc;
use m3::session::DiskOperation;

use super::chan::Channel;
use super::PartDesc;

const PORTBASE_PRIMARY: u16 = 0x1F0;
const PORTBASE_SECONDARY: u16 = 0x170;

const CHAN_PRIMARY: u8 = 0;
const CHAN_SECONDARY: u8 = 1;

const IDE_CTRL_CLASS: u8 = 0x01;
const IDE_CTRL_SUBCLASS: u8 = 0x01;

pub const IDE_CTRL_BAR: usize = 4;

pub const DEVICE_COUNT: usize = 4;

int_enum! {
    pub struct DeviceId : u32 {
        const PRIM_MASTER   = 0x0;
        const PRIM_SLAVE    = 0x1;
        const SEC_MASTER    = 0x2;
        const SEC_SLAVE     = 0x3;
    }
}

int_enum! {
    pub struct ControlFlag : u8 {
        // set this to read back the High Order Byte of the last LBA48 value sent to an IO port.
        const HIGH_ORDER_BYTE = 1 << 7;
        // software reset -- set this to reset all ATA drives on a bus, if one is misbehaving.
        const SOFTWARE_RESET = 1 << 2;
        // set this to stop the current device from sending interrupts.
        const NIEN = 1 << 1;
    }
}

pub struct IDEController {
    chans: Vec<Channel>,
}

impl IDEController {
    pub fn new(use_irq: bool, use_dma: bool) -> Result<Self, Error> {
        // find IDE controller via PCI
        let pci_dev = Rc::new(pci::Device::new("idectrl", kif::TileISA::IDE_DEV)?);
        let mut ide_ctrl = pci_dev.get_info()?;
        assert!(ide_ctrl.class().base() == IDE_CTRL_CLASS);
        assert!(ide_ctrl.class().sub() == IDE_CTRL_SUBCLASS);

        log!(
            crate::LOG_DEF,
            "Found IDE controller ({}): vendor {:x} device {:x} rev {}",
            ide_ctrl.id(),
            ide_ctrl.vendor(),
            ide_ctrl.device(),
            ide_ctrl.revision()
        );

        // ensure that the I/O space is enabled and bus mastering is enabled
        let status_cmd: u32 = pci_dev.read_config(pci::Reg::COMMAND.val)?;
        pci_dev.write_config(pci::Reg::COMMAND.val, (status_cmd & !0x400) | 0x01 | 0x04)?;

        // request I/O ports for bus mastering
        if use_dma && ide_ctrl.bar(IDE_CTRL_BAR).addr() == 0 {
            pci_dev.write_config(pci::Type0::BASE_ADDR4.val, 0x400)?;
            ide_ctrl.bar_mut(IDE_CTRL_BAR).set_addr(0x400);
        }

        // detect channels and devices
        let mut chans = Vec::new();
        let ids = [CHAN_PRIMARY, CHAN_SECONDARY];
        let ports = [PORTBASE_PRIMARY, PORTBASE_SECONDARY];
        for i in 0..2 {
            let dev = pci_dev.clone();
            match Channel::new(dev, &ide_ctrl, use_irq, use_dma, ids[i], ports[i]) {
                Ok(c) => chans.push(c),
                Err(e) => log!(crate::LOG_DEF, "chan[{}] ignoring channel: {}", ids[i], e),
            }
        }

        Ok(Self { chans })
    }

    pub fn channel(&self) -> &Vec<Channel> {
        &self.chans
    }

    pub fn read_write(
        &self,
        part: PartDesc,
        op: DiskOperation,
        buf: &MemGate,
        buf_off: usize,
        disk_off: usize,
        bytes: usize,
    ) -> Result<(), Error> {
        self.chans[part.chan as usize].read_write(part, op, buf, buf_off, disk_off, bytes)
    }
}
