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

#![no_std]

use core::fmt;

use bitflags::bitflags;
use m3::cfg;
use m3::com::{EpMng, MemGate, RecvGate, SendGate, EP};
use m3::errors::Error;
use m3::goff;
use m3::int_enum;
use m3::kif::{Perm, TileDesc, TileISA, TileType};
use m3::tcu::EpId;
use m3::tiles::{ChildActivity, RunningDeviceActivity, Tile};
use m3::util::math;

const EP_INT: EpId = 16;
const EP_DMA: EpId = 17;

// hardcoded for now
const REG_ADDR: goff = 0x4000;
const PCI_CFG_ADDR: goff = 0x0F00_0000;

const MSG_SIZE: usize = 64;
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
    _activity: RunningDeviceActivity,
    mem: MemGate,
    _sep: EP,
    mep: EP,
    rgate: RecvGate,
    _sgate: SendGate,
}

#[derive(Copy, Clone, Debug)]
pub enum BarType {
    Memory,
    IO,
}

impl From<u8> for BarType {
    fn from(val: u8) -> Self {
        match val {
            0 => BarType::Memory,
            _ => BarType::IO,
        }
    }
}

bitflags! {
    pub struct BarFlags : u8 {
        const MEM_32        = 0x1;
        const MEM_64        = 0x2;
        const MEM_PREFETCH  = 0x4;
    }
}

#[derive(Debug)]
pub struct Bar {
    ty: BarType,
    flags: BarFlags,
    addr: usize,
    size: usize,
}

impl Bar {
    pub fn bar_type(&self) -> BarType {
        self.ty
    }

    pub fn flags(&self) -> BarFlags {
        self.flags
    }

    pub fn addr(&self) -> usize {
        self.addr
    }

    pub fn set_addr(&mut self, addr: usize) {
        self.addr = addr
    }

    pub fn size(&self) -> usize {
        self.size
    }
}

#[derive(Copy, Clone, Debug)]
pub struct BDF {
    bus: u8,
    device: u8,
    function: u8,
}

impl BDF {
    pub fn new(bus: u8, device: u8, function: u8) -> Self {
        Self {
            bus,
            device,
            function,
        }
    }

    pub fn bus(&self) -> u8 {
        self.bus
    }

    pub fn device(&self) -> u8 {
        self.device
    }

    pub fn function(&self) -> u8 {
        self.function
    }
}

impl fmt::Display for BDF {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "{}.{}.{}", self.bus, self.device, self.function)
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Class {
    base: u8,
    sub: u8,
}

impl Class {
    pub fn new(base: u8, sub: u8) -> Self {
        Self { base, sub }
    }

    pub fn base(&self) -> u8 {
        self.base
    }

    pub fn sub(&self) -> u8 {
        self.sub
    }
}

#[derive(Debug)]
pub struct Info {
    id: BDF,
    ty: u8,
    device: u16,
    vendor: u16,
    class: Class,
    prog_if: u8,
    revision: u8,
    irq: u8,
    bars: [Bar; 6],
}

impl Info {
    pub fn id(&self) -> BDF {
        self.id
    }

    pub fn device_type(&self) -> u8 {
        self.ty
    }

    pub fn device(&self) -> u16 {
        self.device
    }

    pub fn vendor(&self) -> u16 {
        self.vendor
    }

    pub fn class(&self) -> Class {
        self.class
    }

    pub fn programming_interface(&self) -> u8 {
        self.prog_if
    }

    pub fn revision(&self) -> u8 {
        self.revision
    }

    pub fn interrupt(&self) -> u8 {
        self.irq
    }

    pub fn bar(&self, idx: usize) -> &Bar {
        &self.bars[idx]
    }

    pub fn bar_mut(&mut self, idx: usize) -> &mut Bar {
        &mut self.bars[idx]
    }
}

impl Device {
    pub fn new(name: &str, isa: TileISA) -> Result<Self, Error> {
        let tile = Tile::new(TileDesc::new(TileType::COMP, isa, 0))?;
        let act = ChildActivity::new(tile, name)?;
        let act_sel = act.sel();
        let mem = act.get_mem(
            0,
            (PCI_CFG_ADDR + REG_ADDR) + cfg::PAGE_SIZE as goff,
            Perm::RW,
        )?;
        let sep = EpMng::acquire_for(act_sel, EP_INT, 0)?;
        let mep = EpMng::acquire_for(act_sel, EP_DMA, 0)?;
        let rgate = RecvGate::new(math::next_log2(BUF_SIZE), math::next_log2(MSG_SIZE))?;
        let sgate = SendGate::new(&rgate)?;
        rgate.activate()?;
        sep.configure(sgate.sel())?;

        Ok(Self {
            _activity: act.start()?,
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
        if let Ok(msg) = self.rgate.fetch() {
            self.rgate.ack_msg(msg).unwrap();
            true
        }
        else {
            false
        }
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
            // TODO this is hardcoded atm, because the device tile contains exactly one PCI device
            id: BDF::new(0, 0, 0),
            vendor: self.read_config(Reg::VENDOR_ID.val)?,
            device: self.read_config(Reg::DEVICE_ID.val)?,
            ty: self.read_config(Reg::HEADER_TYPE.val)?,
            revision: self.read_config(Reg::REVISION_ID.val)?,
            prog_if: self.read_config(Reg::CLASS_CODE.val)?,
            class: Class::new(
                self.read_config(Reg::BASE_CLASS_CODE.val)?,
                self.read_config(Reg::SUB_CLASS_CODE.val)?,
            ),
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
            ty: BarType::from((val & 0x1) as u8),
            addr: (val & !0xF) as usize,
            size: size as usize,
            flags,
        })
    }
}
