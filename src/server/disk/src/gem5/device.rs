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

use bitflags::bitflags;

use num_enum::IntoPrimitive;

use m3::cell::StaticRefCell;
use m3::col::Vec;
use m3::com::MemGate;
use m3::errors::{Code, Error};
use m3::io::LogFlags;
use m3::kif::Perm;
use m3::log;
use m3::mem::{self, GlobOff};
use m3::tiles::OwnActivity;
use m3::time::TimeDuration;

use super::chan::Channel;
use super::ctrl::ControlFlag;
use crate::partition::{parse_partitions, Partition};

const ATA_WAIT_TIMEOUT: TimeDuration = TimeDuration::from_micros(500);

const PIO_XFER_TIMEOUT: TimeDuration = TimeDuration::from_millis(3);
const PIO_XFER_SLEEPTIME: TimeDuration = TimeDuration::from_micros(1);

const DMA_XFER_TIMEOUT: TimeDuration = TimeDuration::from_millis(3);
const DMA_XFER_SLEEPTIME: TimeDuration = TimeDuration::from_micros(20);

const SLEEP_TIME: TimeDuration = TimeDuration::from_micros(20);

static BUF: StaticRefCell<[u16; 1024]> = StaticRefCell::new([0; 1024]);

/// ATA I/O ports as offsets from base
#[derive(Copy, Clone, Debug, Eq, PartialEq, IntoPrimitive)]
#[repr(u16)]
pub enum ATAReg {
    Data        = 0x0,
    Error       = 0x1,
    SectorCount = 0x2,
    Address1    = 0x3,
    Address2    = 0x4,
    Address3    = 0x5,
    DriveSelect = 0x6,
    CmdStatus   = 0x7,
    Control     = 0x206,
}

/// ATA commands
#[allow(dead_code)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, IntoPrimitive)]
#[repr(u8)]
enum Command {
    Identify       = 0xEC,
    IdentifyPacket = 0xA1,
    ReadSec        = 0x20,
    ReadSecExt     = 0x24,
    WriteSec       = 0x30,
    WriteSecExt    = 0x34,
    ReadDMA        = 0xC8,
    ReadDMAExt     = 0x25,
    WriteDMA       = 0xCA,
    WriteDMAExt    = 0x35,
    Packet         = 0xA0,
    AtapiReset     = 0x8,
}

bitflags! {
    /// ATA status register
    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    pub struct CommandStatus : u8 {
        /// Drive is preparing to accept/send data -- wait until this bit clears. If it never
        /// clears, do a Software Reset. Technically, when BSY is set, the other bits in the Status
        /// byte are meaningless.
        const BUSY = 1 << 7;
        /// Bit is clear when device is spun down, or after an error. Set otherwise.
        const READY = 1 << 6;
        /// Drive Fault Error (does not set ERR!)
        const DISK_FAULT = 1 << 5;
        /// Overlapped Mode Service Request
        const OVERLAPPED_REQ = 1 << 4;
        /// Set when the device has PIO data to transfer, or is ready to accept PIO data.
        const DRQ = 1 << 3;
        /// Error flag (when set). Send a new command to clear it (or nuke it with a Software
        /// Reset).
        const ERROR = 1 << 0;
    }
}

bitflags! {
    /// ATA device capabilities
    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    struct Capabilities : u16 {
        const DMA = 1 << 8;
        const LBA = 1 << 9;
        const IORDY_DISABLED = 1 << 10;
        const IORDY_SUPPORTED = 1 << 11;
    }
}

bitflags! {
    /// ATA device features
    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    struct Features : u32 {
        const SMART = 1 << 0;
        const SECURITY_MODE = 1 << 1;
        const REMOVABLE_MEDIA = 1 << 2;
        const POWER_MANAGEMENT = 1 << 3;
        const PACKET = 1 << 4;
        const WRITE_CACHE = 1 << 5;
        const LOOK_AHEAD = 1 << 6;
        const RELEASE_INT = 1 << 7;
        const SERVICE_INT = 1 << 8;
        const DEVICE_RESET = 1 << 9;
        const HOST_PROT_AREA = 1 << 10;
        const WRITE_BUFFER = 1 << 12;
        const READ_BUFFER = 1 << 13;
        const NOP = 1 << 14;
        const DOWNLOAD_MICROCODE = 1 << 16;
        const RW_DMA_QUEUED = 1 << 17;
        const CFA = 1 << 18;
        const APM = 1 << 19;
        const REMOVABLE_MEDIA_SN = 1 << 20;
        const POWERUP_STANDBY = 1 << 21;
        const SET_FEATURES_SPINUP = 1 << 22;
        const SET_MAX_SECURITY = 1 << 24;
        const AUTO_ACOUSTIC_MNG = 1 << 25;
        const LBA48 = 1 << 26;
        const DEV_CFG_OVERLAY = 1 << 27;
        const FLUSH_CACHE = 1 << 28;
        const FLUSH_CACHE_EXT = 1 << 29;
    }
}

/// Bus master IDE registers
#[derive(Copy, Clone, Debug, Eq, PartialEq, IntoPrimitive)]
#[repr(u16)]
pub enum BMIReg {
    Command = 0x0,
    Status  = 0x2,
    PRDT    = 0x4,
}

bitflags! {
    /// Bus master IDE status flags
    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    struct BMIStatus : u8 {
        const IRQ = 1 << 2;
        const ERROR = 1 << 1;
        const DMA = 1 << 0;
    }
}

bitflags! {
    /// Bus master IDE commands
    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    struct BMICmd : u8 {
        const START = 1 << 0;
        const READ = 1 << 3;
    }
}

/// physical region descriptor
#[repr(C, packed)]
pub struct PRD {
    buffer: u32,
    bytes: u16,
    last: u16,
}

/// device operations
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum DevOp {
    READ,
    WRITE,
    PACKET,
}

/// Represents an ATA/ATAPI device
pub struct Device {
    id: u8,
    caps: Capabilities,
    features: Features,
    capacity: usize,
    sec_size: usize,
    parts: Vec<Partition>,
}

impl Device {
    pub fn new(id: u8, chan: &Channel) -> Result<Self, Error> {
        // send IDENTIFY command to device
        let mut dev = match Self::identify(id, chan) {
            Err(e) => {
                log!(
                    LogFlags::DiskDev,
                    "chan[{}] command {:?} failed: {}",
                    chan.id(),
                    Command::Identify,
                    e
                );
                return Err(e);
            },

            Ok((caps, features, capacity)) => Self {
                id,
                caps,
                features,
                capacity: capacity as usize,
                sec_size: 512,
                parts: Vec::new(),
            },
        };

        // TODO support ATAPI devices
        if dev.is_atapi() {
            return Err(Error::new(Code::NotSup));
        }

        // read MBR from disk
        let mut buffer = [0u8; 512];
        let size = mem::size_of_val(&buffer) + mem::size_of::<PRD>();
        let mg_buf = MemGate::new(size, Perm::RW)?;
        let dev_buf = mg_buf.derive_cap(0, size, Perm::RW)?;
        chan.set_dma_buffer(&dev_buf)?;
        dev.read_write(chan, DevOp::READ, &mg_buf, 0, 0, dev.sec_size, 1)?;

        // parse partition table
        mg_buf.read(&mut buffer, 0)?;
        for p in parse_partitions(&buffer) {
            if p.present() {
                dev.parts.push(p);
            }
        }

        Ok(dev)
    }

    pub fn id(&self) -> u8 {
        self.id
    }

    pub fn is_atapi(&self) -> bool {
        self.sec_size == 2048
    }

    pub fn use_dma(&self, chan: &Channel) -> bool {
        self.caps.contains(Capabilities::DMA) && chan.use_dma()
    }

    pub fn use_lba48(&self) -> bool {
        self.features.contains(Features::LBA48)
    }

    pub fn size(&self) -> usize {
        self.capacity * self.sec_size
    }

    pub fn sector_size(&self) -> usize {
        self.sec_size
    }

    pub fn partitions(&self) -> &Vec<Partition> {
        &self.parts
    }

    #[allow(clippy::too_many_arguments)]
    pub fn read_write(
        &self,
        chan: &Channel,
        op: DevOp,
        buf: &MemGate,
        off: usize,
        lba: u64,
        sec_size: usize,
        sec_count: usize,
    ) -> Result<(), Error> {
        let cmd = self.get_command(chan, op);

        log!(
            LogFlags::DiskDev,
            "chan[{}] {:?} for sectors {}..{} with {}B sectors",
            chan.id(),
            op,
            lba,
            lba + sec_count as u64 - 1,
            sec_size,
        );

        self.setup_command(chan, lba, sec_count, cmd)?;

        match cmd {
            Command::Packet
            | Command::ReadSec
            | Command::ReadSecExt
            | Command::WriteSec
            | Command::WriteSecExt => {
                self.transfer_pio(chan, op, buf, off, sec_size, sec_count, true)
            },
            _ => self.transfer_dma(chan, op, buf, off, sec_size, sec_count),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn transfer_pio(
        &self,
        chan: &Channel,
        op: DevOp,
        buf: &MemGate,
        off: usize,
        sec_size: usize,
        sec_count: usize,
        wait_first: bool,
    ) -> Result<(), Error> {
        let mut buffer = BUF.borrow_mut();
        for i in 0..sec_count {
            if i > 0 || wait_first {
                if op == DevOp::READ {
                    chan.wait_irq()?;
                }
                chan.wait_until(
                    PIO_XFER_TIMEOUT,
                    PIO_XFER_SLEEPTIME,
                    CommandStatus::READY,
                    CommandStatus::BUSY,
                )?;
            }

            match op {
                DevOp::READ => {
                    chan.read_pio_words(ATAReg::Data, &mut buffer[0..sec_size / 2])?;
                    buf.write(&buffer[0..sec_size / 2], (off + i * sec_size) as GlobOff)?;
                },
                _ => {
                    buf.read(
                        &mut buffer[0..sec_size / 2],
                        (off + i * sec_size) as GlobOff,
                    )?;
                    chan.write_pio_words(ATAReg::Data, &buffer[0..sec_size / 2])?;
                },
            }
        }

        Ok(())
    }

    fn transfer_dma(
        &self,
        chan: &Channel,
        op: DevOp,
        buf: &MemGate,
        off: usize,
        sec_size: usize,
        sec_count: usize,
    ) -> Result<(), Error> {
        // setup PRDT
        let prdt = PRD {
            buffer: 0,
            bytes: (sec_count * sec_size) as u16,
            last: 1 << 15,
        };
        // write it behind the buffer
        buf.write(&[prdt], (off + sec_size * sec_count) as GlobOff)?;

        // stop running transfers
        chan.write_bmr::<u8>(BMIReg::Command, 0)?;
        let status = chan.read_bmr::<u8>(BMIReg::Status)?;
        chan.write_bmr::<u8>(
            BMIReg::Status,
            status | BMIStatus::ERROR.bits() | BMIStatus::IRQ.bits(),
        )?;

        // set PRDT
        chan.write_bmr::<u32>(BMIReg::PRDT, (sec_size * sec_count) as u32)?;

        // it seems to be necessary to read those ports here
        chan.read_bmr::<u8>(BMIReg::Command)?;
        chan.read_bmr::<u8>(BMIReg::Status)?;
        // start bus mastering
        if op == DevOp::READ {
            chan.write_bmr::<u8>(BMIReg::Command, (BMICmd::START | BMICmd::READ).bits())?;
        }
        else {
            chan.write_bmr::<u8>(BMIReg::Command, BMICmd::START.bits())?;
        }
        chan.read_bmr::<u8>(BMIReg::Command)?;
        chan.read_bmr::<u8>(BMIReg::Status)?;

        // wait for an interrupt
        chan.wait_irq()?;

        chan.wait_until(
            DMA_XFER_TIMEOUT,
            DMA_XFER_SLEEPTIME,
            CommandStatus::empty(),
            CommandStatus::BUSY | CommandStatus::DRQ,
        )?;

        chan.read_bmr::<u8>(BMIReg::Status)?;
        chan.write_bmr::<u8>(BMIReg::Command, 0)
    }

    fn setup_command(
        &self,
        chan: &Channel,
        lba: u64,
        sec_count: usize,
        cmd: Command,
    ) -> Result<(), Error> {
        if sec_count == 0 {
            return Err(Error::new(Code::InvArgs));
        }

        if self.use_lba48() {
            chan.select(self.id, 0x40)?;
        }
        else {
            if (lba & 0xFFFF_FFFF_F000_0000) != 0 || (sec_count & 0xFF00) != 0 {
                return Err(Error::new(Code::NotSup));
            }
            // for LBA28, the lowest 4 bits are bits 27-24 of LBA
            chan.select(self.id, 0x40 | ((lba >> 24) & 0x0F) as u8)?;
        }

        // reset control register
        let nien = if chan.use_irq() {
            0
        }
        else {
            ControlFlag::NIEN.into()
        };
        chan.write_pio::<u8>(ATAReg::Control, nien)?;

        log!(
            LogFlags::DiskDev,
            "chan[{}] setting LBA={}, sec_count={}",
            chan.id(),
            lba,
            sec_count
        );

        if self.use_lba48() {
            // LBA: | LBA6 | LBA5 | LBA4 | LBA3 | LBA2 | LBA1 |
            //     48             32            16            0
            // sector count, high byte
            chan.write_pio::<u8>(ATAReg::SectorCount, (sec_count >> 8) as u8)?;
            // LBA4, LBA5, and LBA6
            chan.write_pio::<u8>(ATAReg::Address1, (lba >> 24) as u8)?;
            chan.write_pio::<u8>(ATAReg::Address2, (lba >> 32) as u8)?;
            chan.write_pio::<u8>(ATAReg::Address3, (lba >> 40) as u8)?;
            // sector count, low byte
            chan.write_pio::<u8>(ATAReg::SectorCount, (sec_count & 0xFF) as u8)?;
        }
        else {
            // sector count
            chan.write_pio::<u8>(ATAReg::SectorCount, sec_count as u8)?;
        }

        // LBA1, LBA2, and LBA3
        chan.write_pio::<u8>(ATAReg::Address1, (lba & 0xFF) as u8)?;
        chan.write_pio::<u8>(ATAReg::Address2, (lba >> 8) as u8)?;
        chan.write_pio::<u8>(ATAReg::Address3, (lba >> 16) as u8)?;

        log!(
            LogFlags::DiskDev,
            "chan[{}] starting command {:?}",
            chan.id(),
            cmd
        );

        // send command
        chan.write_pio::<u8>(ATAReg::CmdStatus, cmd as u8)
    }

    fn get_command(&self, chan: &Channel, op: DevOp) -> Command {
        if op == DevOp::PACKET {
            return Command::Packet;
        }

        let cmds = [
            Command::ReadSec,
            Command::ReadSecExt,
            Command::WriteSec,
            Command::WriteSecExt,
            Command::ReadDMA,
            Command::ReadDMAExt,
            Command::WriteDMA,
            Command::WriteDMAExt,
        ];

        let mut idx = if self.use_dma(chan) { 4 } else { 0 };
        if self.use_lba48() {
            idx += 1;
        }
        if op == DevOp::WRITE {
            idx += 2;
        }
        cmds[idx]
    }

    fn identify(id: u8, chan: &Channel) -> Result<(Capabilities, Features, u32), Error> {
        // select device
        chan.select(id, 0)?;

        // disable interrupts
        chan.write_pio(ATAReg::Control, ControlFlag::NIEN as u8)?;

        // check whether the device exists
        log!(
            LogFlags::DiskDev,
            "chan[{}] sending '{:?}' to device {}",
            chan.id(),
            Command::Identify,
            id
        );
        chan.write_pio(ATAReg::CmdStatus, Command::Identify as u8)?;

        let status: u8 = chan.read_pio(ATAReg::CmdStatus)?;
        if status == 0 {
            Err(Error::new(Code::NotFound))
        }
        else {
            // TODO from the osdev wiki: Because of some ATAPI drives that do not follow spec, at
            // this point you need to check the LBAmid and LBAhi ports (0x1F4 and 0x1F5) to see if
            // they are non-zero. If so, the drive is not ATA, and you should stop polling.

            let mut elapsed = TimeDuration::ZERO;
            while elapsed < ATA_WAIT_TIMEOUT
                && (chan.read_pio::<u8>(ATAReg::CmdStatus)? & CommandStatus::BUSY.bits()) != 0
            {
                OwnActivity::sleep_for(SLEEP_TIME)?;
                elapsed += SLEEP_TIME;
            }
            chan.wait();

            // wait until ready or error
            chan.wait_until(
                ATA_WAIT_TIMEOUT,
                SLEEP_TIME,
                CommandStatus::DRQ,
                CommandStatus::BUSY,
            )?;

            // device is ready, read data
            let mut words = [0u16; 256];
            chan.read_pio_words(ATAReg::Data, &mut words)?;

            // wait until DRQ and BUSY bits are unset
            chan.wait_until(
                ATA_WAIT_TIMEOUT,
                SLEEP_TIME,
                CommandStatus::empty(),
                CommandStatus::DRQ | CommandStatus::BUSY,
            )?;

            let caps = Capabilities::from_bits_truncate(words[49]);
            let feature_bits = words[75] as u32 | ((words[76] as u32) << 16);
            let features = Features::from_bits_truncate(feature_bits);
            let capacity = words[60] as u32 | ((words[61] as u32) << 16);
            Ok((caps, features, capacity))
        }
    }
}
