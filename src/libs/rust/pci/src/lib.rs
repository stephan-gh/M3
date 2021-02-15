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

#![no_std]

use bitflags::bitflags;
use m3::cfg;
use m3::com::{MemGate, RecvGate, SendGate, EP};
use m3::errors::Error;
use m3::goff;
use m3::int_enum;
use m3::kif::{PEDesc, PEType, Perm, PEISA};
use m3::math;
use m3::pes::{DeviceActivity, PE, VPE};
use m3::tcu::EpId;

const EP_INT: EpId = 16;
const EP_DMA: EpId = 17;

// hardcoded for now
const REG_ADDR: goff = 0x4000;
const PCI_CFG_ADDR: goff = 0x0F00_0000;

const MSG_SIZE: usize = 32;
const BUF_SIZE: usize = MSG_SIZE * 8;

// Common PCI offsets
int_enum! {
    pub struct Reg : goff {
        const VENDOR_ID = 0x00;       // Vendor ID                    ro
        const DEVICE_ID = 0x02;       // Device ID                    ro
        const COMMAND = 0x04;         // Command                      rw
        const STATUS = 0x06;          // Status                       rw
        const REVISION_ID = 0x08;     // Revision ID                  ro
        const CLASS_CODE = 0x09;      // Class Code                   ro
        const SUB_CLASS_CODE = 0x0A;  // Sub Class Code               ro
        const BASE_CLASS_CODE = 0x0B; // Base Class Code              ro
        const CACHE_LINE_SIZE = 0x0C; // Cache Line Size              ro+
        const LATENCY_TIMER = 0x0D;   // Latency Timer                ro+
        const HEADER_TYPE = 0x0E;     // Header Type                  ro
        const BIST = 0x0F;            // Built in self test           rw
    }
}

// Type 0 PCI offsets
int_enum! {
    pub struct Type0 : goff {
        const BASE_ADDR0 = 0x10;      // Base Address 0               rw
        const BASE_ADDR1 = 0x14;      // Base Address 1               rw
        const BASE_ADDR2 = 0x18;      // Base Address 2               rw
        const BASE_ADDR3 = 0x1C;      // Base Address 3               rw
        const BASE_ADDR4 = 0x20;      // Base Address 4               rw
        const BASE_ADDR5 = 0x24;      // Base Address 5               rw
        const CIS = 0x28;             // CardBus CIS Pointer          ro
        const SUB_VENDOR_ID = 0x2C;   // Sub-Vendor ID                ro
        const SUB_SYSTEM_ID = 0x2E;   // Sub-System ID                ro
        const ROM_BASE_ADDR = 0x30;   // Expansion ROM Base Address   rw
        const CAP_PTR = 0x34;         // Capability list pointer      ro
        const RESERVED = 0x35;
        const INTERRUPT_LINE = 0x3C;  // Interrupt Line               rw
        const INTERRUPT_PIN = 0x3D;   // Interrupt Pin                ro
        const MIN_GRANT = 0x3E;       // Maximum Grant                ro
        const MAX_LATENCY = 0x3F;     // Maximum Latency              ro
    }
}

pub struct Device {
    _activity: DeviceActivity,
    mem: MemGate,
    _sep: EP,
    mep: EP,
    rgate: RecvGate,
    _sgate: SendGate,
}

int_enum! {
    pub struct BarType : u8 {
        const MEM   = 0x0;
        const IO    = 0x1;
    }
}

bitflags! {
    pub struct BarFlags : u8 {
        const MEM_32        = 0x1;
        const MEM_64        = 0x2;
        const MEM_PREFETCH  = 0x4;
    }
}

pub struct Bar {
    pub ty: u8,
    pub flags: BarFlags,
    pub addr: usize,
    pub size: usize,
}

pub struct Info {
    pub bus: u8,
    pub dev: u8,
    pub func: u8,
    pub ty: u8,
    pub dev_id: u16,
    pub vendor_id: u16,
    pub base_class: u8,
    pub sub_class: u8,
    pub prog_if: u8,
    pub rev_id: u8,
    pub irq: u8,
    pub bars: [Bar; 6],
}

impl Device {
    pub fn new(name: &str, isa: PEISA) -> Result<Self, Error> {
        let pe = PE::new(PEDesc::new(PEType::COMP_IMEM, isa, 0))?;
        let vpe = VPE::new(pe, name)?;
        let vpe_sel = vpe.sel();
        let mem = vpe.get_mem(
            0,
            (PCI_CFG_ADDR + REG_ADDR) + cfg::PAGE_SIZE as goff,
            Perm::RW,
        )?;
        let sep = vpe.epmng().acquire_for(vpe_sel, EP_INT, 0)?;
        let mep = vpe.epmng().acquire_for(vpe_sel, EP_DMA, 0)?;
        let mut rgate = RecvGate::new(math::next_log2(BUF_SIZE), math::next_log2(MSG_SIZE))?;
        let sgate = SendGate::new(&rgate)?;
        rgate.activate()?;
        sep.configure(sgate.sel())?;

        Ok(Self {
            _activity: vpe.start()?,
            mem,
            _sep: sep,
            mep,
            rgate,
            _sgate: sgate,
        })
    }

    pub fn set_dma_buffer(&self, mgate: &MemGate) -> Result<(), Error> {
        self.mep.configure(mgate.sel())
    }

    pub fn check_for_irq(&self) -> bool {
        self.rgate.fetch().is_some()
    }

    pub fn wait_for_irq(&self) -> Result<(), Error> {
        self.rgate
            .receive(None)
            .and_then(|msg| self.rgate.ack_msg(msg))
    }

    pub fn read_reg<T>(&self, off: goff) -> Result<T, Error> {
        self.mem.read_obj(REG_ADDR + off)
    }

    pub fn write_reg<T>(&self, off: goff, val: T) -> Result<(), Error> {
        self.mem.write_obj(&val, REG_ADDR + off)
    }

    pub fn read_config<T>(&self, off: goff) -> Result<T, Error> {
        self.mem.read_obj(REG_ADDR + PCI_CFG_ADDR + off)
    }

    pub fn write_config<T>(&self, off: goff, val: T) -> Result<(), Error> {
        self.mem.write_obj(&val, REG_ADDR + PCI_CFG_ADDR + off)
    }

    pub fn get_info(&self) -> Result<Info, Error> {
        Ok(Info {
            // TODO this is hardcoded atm, because the device PE contains exactly one PCI device
            bus: 0,
            dev: 0,
            func: 0,
            vendor_id: self.read_config(Reg::VENDOR_ID.val)?,
            dev_id: self.read_config(Reg::DEVICE_ID.val)?,
            ty: self.read_config(Reg::HEADER_TYPE.val)?,
            rev_id: self.read_config(Reg::REVISION_ID.val)?,
            prog_if: self.read_config(Reg::CLASS_CODE.val)?,
            base_class: self.read_config(Reg::BASE_CLASS_CODE.val)?,
            sub_class: self.read_config(Reg::SUB_CLASS_CODE.val)?,
            irq: 0,
            bars: [
                self.read_bar(0)?,
                self.read_bar(1)?,
                self.read_bar(2)?,
                self.read_bar(3)?,
                self.read_bar(4)?,
                self.read_bar(5)?,
            ],
        })
    }

    fn read_bar(&self, idx: usize) -> Result<Bar, Error> {
        let val: u32 = self.read_config(Type0::BASE_ADDR0.val + idx as goff * 4)?;
        self.write_config(
            Type0::BASE_ADDR0.val + idx as goff * 4,
            0xFFFF_FFF0 | (val & 0x1),
        )?;

        let mut flags = BarFlags::empty();
        let mut size: u32 = self.read_config(Type0::BASE_ADDR0.val + idx as goff * 4)?;
        let size = if size == 0 || size == 0xFFFF_FFFF {
            0
        }
        else {
            // memory bar?
            if (size & 0x1) == 0 {
                match (val >> 1) & 0x3 {
                    0 => flags |= BarFlags::MEM_32,
                    2 => flags |= BarFlags::MEM_64,
                    _ => panic!("Unexpected BAR value {:x}", val),
                }
                if ((val >> 3) & 0x1) != 0 {
                    flags |= BarFlags::MEM_PREFETCH;
                }
                size &= 0xFFFF_FFFC;
            }
            // IO bar
            else {
                size &= 0xFFFF_FFF0;
            }
            size & (size - 1)
        };
        self.write_config(0x10 + idx as goff * 4, val)?;

        Ok(Bar {
            ty: (val & 0x1) as u8,
            addr: (val & !0xF) as usize,
            size: size as usize,
            flags,
        })
    }
}
