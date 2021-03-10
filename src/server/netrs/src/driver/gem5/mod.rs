/*
 * Copyright (C) 2021, Tendsin Mende <tendsin.mende@mailbox.tu-dresden.de>
 * Copyright (C) 2017, Georg Kotheimer <georg.kotheimer@mailbox.tu-dresden.de>
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

use m3::cell::RefCell;
use m3::com::MemGate;
use m3::errors::{Code, Error};
use m3::goff;
use m3::kif::{Perm, PEISA};
use m3::net::{MAC, MAC_LEN};
use m3::rc::Rc;
use m3::vec::Vec;
use m3::{log, vec};

use memoffset::offset_of;

use pci::Device;

use smoltcp::time::Instant;

pub mod defines;
use defines::*;

#[inline]
fn inc_rb(index: u32, size: u32) -> u32 {
    (index + 1) % size
}

struct EEPROM {
    shift: i32,
    done_bit: u32,
}

impl EEPROM {
    fn new(device: &Device) -> Result<Self, Error> {
        device.write_reg(e1000::REG::EERD.bits(), e1000::EERD::START.bits() as u32)?;

        let t = base::tcu::TCU::nanotime();
        let mut tried_once = false;
        while !tried_once && (base::tcu::TCU::nanotime() - t) < MAX_WAIT_NANOS {
            let value: u32 = device.read_reg(e1000::REG::EERD.bits())?;

            if (value & e1000::EERD::DONE_LARGE.bits() as u32) > 0 {
                log!(crate::LOG_NIC, "Detected large EERD");
                return Ok(Self {
                    shift: e1000::EERD::SHIFT_LARGE.bits().into(),
                    done_bit: e1000::EERD::DONE_LARGE.bits().into(),
                });
            }
            if (value & e1000::EERD::DONE_SMALL.bits() as u32) > 0 {
                log!(crate::LOG_NIC, "Detected small EERD");
                return Ok(Self {
                    shift: e1000::EERD::SHIFT_SMALL.bits().into(),
                    done_bit: e1000::EERD::DONE_SMALL.bits().into(),
                });
            }
            tried_once = true;
        }

        log!(crate::LOG_NIC, "Timeout while trying to create EEPROM");
        Err(Error::new(Code::Timeout))
    }

    //reads `data` of `len` from the device.
    //TOD: Currently doing stuff with the ptr of data. Should probably give sub slices of the length of one
    // word tp the read_word fct. Also `len` is not needed since rust slice know their length.
    fn read(&self, dev: &E1000, mut address: usize, mut data: &mut [u8]) -> Result<(), Error> {
        assert!((data.len() & ((1 << WORD_LEN_LOG2) - 1)) == 0);

        let num_bytes_to_move = 1 << WORD_LEN_LOG2;
        let mut len = data.len();
        while len > 0 {
            self.read_word(dev, address, data)?;
            //move to next word
            data = &mut data[num_bytes_to_move..];
            address += 1;
            len -= num_bytes_to_move;
        }
        Ok(())
    }

    fn read_word(&self, dev: &E1000, address: usize, data: &mut [u8]) -> Result<(), Error> {
        //cast to 16bit array
        let data_word: &mut [u16] = unsafe { core::mem::transmute::<&mut [u8], &mut [u16]>(data) };

        //set address
        dev.write_reg(
            e1000::REG::EERD,
            e1000::EERD::START.bits() as u32 | (address << self.shift) as u32,
        );

        //Wait for read to complete
        let t = base::tcu::TCU::nanotime();
        let mut done_once = false;
        while (base::tcu::TCU::nanotime() - t) < MAX_WAIT_NANOS && !done_once {
            let value = dev.read_reg(e1000::REG::EERD);
            done_once = true;
            if (!value & self.done_bit) != 0 {
                //Not read yet, therefore try again
                continue;
            }
            //Move word into slice
            data_word[0] = (value >> 16) as u16;
            return Ok(());
        }

        Err(Error::new(Code::Timeout))
    }
}

struct E1000 {
    nic: Device,
    eeprom: EEPROM,
    mac: MAC,

    cur_tx_desc: u32,
    cur_tx_buf: u32,

    bufs: MemGate,
    devbufs: MemGate,

    link_state_changed: bool,
    txd_context_proto: TxoProto,
}

static ZEROS: [u8; 4096] = [0; 4096];

impl E1000 {
    pub fn new() -> Result<Self, Error> {
        log!(crate::LOG_NIC, "Creating NIC");
        let nic = Device::new("nic", PEISA::NIC_DEV)?;

        //TODO create global memory in rust, should be global...?
        let bufs = MemGate::new(core::mem::size_of::<Buffers>(), Perm::RW)?;
        let devbufs = bufs.derive(0, core::mem::size_of::<Buffers>(), Perm::RW)?;

        let mut dev = E1000 {
            eeprom: EEPROM::new(&nic)?,
            nic,
            mac: MAC::broadcast(), //gets initialised at reset

            cur_tx_desc: 0,
            cur_tx_buf: 0,

            bufs,
            devbufs,

            link_state_changed: true,
            txd_context_proto: TxoProto::UNSUPPORTED,
        };

        dev.nic.set_dma_buffer(&dev.devbufs)?;

        //clear descriptors
        let mut i = 0;
        while i < core::mem::size_of::<Buffers>() {
            dev.write_bufs(
                &ZEROS,
                (core::mem::size_of::<Buffers>() - i)
                    .min(core::mem::size_of_val(&ZEROS))
                    .min(i) as goff,
            );
            i += core::mem::size_of_val(&ZEROS);
        }

        //Reset card
        dev.reset();

        //Enable interrupts
        dev.write_reg(
            e1000::REG::IMC,
            (e1000::ICR::LSC | e1000::ICR::RXO | e1000::ICR::RXT0)
                .bits()
                .into(),
        );
        dev.write_reg(
            e1000::REG::IMS,
            (e1000::ICR::LSC | e1000::ICR::RXO | e1000::ICR::RXT0)
                .bits()
                .into(),
        );

        Ok(dev)
    }

    fn reset(&mut self) {
        //always reset MAC. Required to reset the TX and RX rings.
        let mut ctrl: u32 = self.read_reg(e1000::REG::CTRL);
        self.write_reg(e1000::REG::CTRL, ctrl | e1000::CTL::RESET.bits());
        self.sleep(RESET_SLEEP_TIME);

        //set a sensible default configuration
        ctrl |= (e1000::CTL::SLU | e1000::CTL::ASDE).bits();
        ctrl &= (e1000::CTL::LRST | e1000::CTL::FRCSPD | e1000::CTL::FRCDPLX).bits();
        self.write_reg(e1000::REG::CTRL, ctrl);
        self.sleep(RESET_SLEEP_TIME);

        // if link is already up, do not attempt to reset the PHY.  On
        // some models (notably ICH), performing a PHY reset seems to
        // drop the link speed to 10Mbps.
        let status: u32 = self.read_reg(e1000::REG::STATUS);
        if ((!status) & e1000::STATUS::LU.bits() as u32) > 0 {
            // Reset PHY and MAC simultaneously
            self.write_reg(
                e1000::REG::CTRL,
                ctrl | (e1000::CTL::RESET | e1000::CTL::PHY_RESET).bits(),
            );
            self.sleep(RESET_SLEEP_TIME);

            // PHY reset is not self-clearing on all models
            self.write_reg(e1000::REG::CTRL, ctrl);
            self.sleep(RESET_SLEEP_TIME);
        }

        // enable ip/udp/tcp receive checksum offloading
        self.write_reg(
            e1000::REG::RXCSUM,
            (e1000::RXCSUM::IPOFLD | e1000::RXCSUM::TUOFLD)
                .bits()
                .into(),
        );

        // calculate field offsets. needs to happen in const to not instantiate `Buffers`.
        const RX_BUF_OFF: usize = offset_of!(Buffers, rx_buf);
        const TX_DESCS_OFF: usize = offset_of!(Buffers, tx_descs);
        const RX_DESCS_OFF: usize = offset_of!(Buffers, rx_descs);

        // setup rx descriptors
        for i in 0..RX_BUF_COUNT {
            //Init rxdesc which is written
            let desc = RxDesc {
                buffer: (RX_BUF_OFF + i * RX_BUF_SIZE) as u64,
                length: RX_BUF_SIZE as u16,
                checksum: 0,
                status: 0,
                error: 0,
                pad: 0,
            };
            self.write_bufs(
                &[desc],
                (RX_DESCS_OFF + i * core::mem::size_of::<RxDesc>()) as goff,
            );

            let mut desc2 = [RxDesc {
                buffer: 0,
                length: 0,
                checksum: 0,
                status: 0,
                error: 0,
                pad: 0,
            }];
            self.read_bufs(
                &mut desc2,
                (RX_DESCS_OFF + i * core::mem::size_of::<RxDesc>()) as goff,
            );
        }

        // init receive ring
        self.write_reg(e1000::REG::RDBAH, 0);
        self.write_reg(e1000::REG::RDBAL, RX_DESCS_OFF as u32);
        self.write_reg(
            e1000::REG::RDLEN,
            (RX_BUF_COUNT * core::mem::size_of::<RxDesc>()) as u32,
        );
        self.write_reg(e1000::REG::RDH, 0);
        self.write_reg(e1000::REG::RDT, (RX_BUF_COUNT - 1) as u32);
        self.write_reg(e1000::REG::RDTR, 0);
        self.write_reg(e1000::REG::RADV, 0);

        // init transmit ring
        self.write_reg(e1000::REG::TDBAH, 0);
        self.write_reg(e1000::REG::TDBAL, TX_DESCS_OFF as u32);
        self.write_reg(
            e1000::REG::TDLEN,
            (TX_BUF_COUNT * core::mem::size_of::<TxDesc>()) as u32,
        );
        self.write_reg(e1000::REG::TDH, 0);
        self.write_reg(e1000::REG::TDT, 0);
        self.write_reg(e1000::REG::TIDV, 0);
        self.write_reg(e1000::REG::TADV, 0);

        // enable rings
        // Always enabled for this model? legacy stuff?
        // self.write_reg(e1000::REG::RDCTL, self.read_reg(e1000::REG::RDCTL) | e1000::XDCTL::ENABLE);
        // self.write_reg(e1000::REG::TDCTL, self.read_reg(e1000::REG::TDCTL) | e1000::XDCTL::ENABLE);

        // get MAC and setup MAC filter
        self.mac = self.read_mac();
        let macval: u64 = self.mac.value();
        self.write_reg(e1000::REG::RAL, (macval & 0xFFFFFFFF) as u32);
        self.write_reg(
            e1000::REG::RAH,
            (((macval >> 32) as u32) & 0xFFFF) | (e1000::RAH::VALID.bits() as u32),
        );

        // enable transmitter
        let mut tctl: u32 = self.read_reg(e1000::REG::TCTL);
        tctl &= !((e1000::TCTL::COLT_MASK | e1000::TCTL::COLD_MASK).bits() as u32);
        tctl |= (e1000::TCTL::ENABLE
            | e1000::TCTL::PSP
            | e1000::TCTL::COLL_DIST
            | e1000::TCTL::COLL_TSH)
            .bits() as u32;
        self.write_reg(e1000::REG::TCTL, tctl);

        // enable receiver
        let mut rctl: u32 = self.read_reg(e1000::REG::RCTL);
        rctl &= !((e1000::RCTL::BSIZE_MASK | e1000::RCTL::BSEX_MASK).bits() as u32);
        rctl |= (e1000::RCTL::ENABLE
            | e1000::RCTL::UPE
            | e1000::RCTL::MPE
            | e1000::RCTL::BAM
            | e1000::RCTL::BSIZE_2K
            | e1000::RCTL::SECRC)
            .bits() as u32;
        self.write_reg(e1000::REG::RCTL, rctl);

        self.link_state_changed = true;
    }

    fn send(&mut self, packet: &[u8]) -> bool {
        assert!(
            packet.len() < E1000::mtu(),
            "Package was too big for E1000 device"
        );

        let mut next_tx_desc: u32 = inc_rb(self.cur_tx_desc, TX_BUF_COUNT as u32);

        let head: u32 = self.read_reg(e1000::REG::TDH);
        // TODO: Is the condition correct or off by one?
        if next_tx_desc == head {
            log!(crate::LOG_NIC, "No free descriptors.");
            return false;
        }

        //Check which protocol is used, ip, tcp, udp.
        let (is_ip, mut txo_proto) = {
            let ethf = smoltcp::wire::EthernetFrame::new_unchecked(packet);
            if ethf.ethertype() == smoltcp::wire::EthernetProtocol::Ipv4 {
                (true, TxoProto::IP)
            }
            else {
                (false, TxoProto::UNSUPPORTED)
            }
        };

        if (txo_proto == TxoProto::IP)
            && ((core::mem::size_of::<EthHdr>() + core::mem::size_of::<IpHdr>()) < packet.len())
        {
            let proto: u8 = unsafe {
                let hdr = (packet as *const _ as *const u8)
                    .offset(core::mem::size_of::<EthHdr>() as isize)
                    as *const IpHdr;
                (*hdr).proto
            };
            if proto == IP_PROTO_TCP {
                txo_proto = TxoProto::TCP;
            }
            else if proto == IP_PROTO_UDP {
                txo_proto = TxoProto::UDP;
            }
        };

        let is_tcp = txo_proto == TxoProto::TCP;
        let is_udp = txo_proto == TxoProto::UDP;

        //check if the type of package has changed, in that case update the context
        let txd_context_update_required: bool = (self.txd_context_proto & txo_proto) != txo_proto;
        if txd_context_update_required {
            next_tx_desc = inc_rb(next_tx_desc, TX_BUF_COUNT as u32);
        }

        if txd_context_update_required && (next_tx_desc == head) {
            log!(
                crate::LOG_NIC,
                "Not enough free descriptors to update context and transmit data."
            );
            return false;
        }

        // calculate field offsets. needs to happen in const to not instantiate `Buffers`.
        const TX_BUF_OFF: usize = offset_of!(Buffers, tx_buf);
        const TX_DESCS_OFF: usize = offset_of!(Buffers, tx_descs);

        //swap tx desc
        let mut cur_tx_desc: u32 = self.cur_tx_desc;
        self.cur_tx_desc = next_tx_desc;

        let cur_tx_buf: u32 = self.cur_tx_buf;
        self.cur_tx_buf = inc_rb(self.cur_tx_buf, TX_BUF_COUNT as u32);

        // Update context descriptor if necessary (different protocol)
        if txd_context_update_required {
            log!(crate::LOG_NIC, "Writing context descriptor.");

            let mut desc = TxContextDesc {
                ipcss: 0,
                ipcso: (core::mem::size_of::<EthHdr>() + offset_of!(IpHdr, chksum)) as u8,
                ipcse: 0,
                tucss: 0,
                tucso: (core::mem::size_of::<EthHdr>()
                    + core::mem::size_of::<IpHdr>()
                    + (if is_tcp {
                        TCP_CHECKSUM_OFFSET
                    }
                    else {
                        UDP_CHECKSUM_OFFSET
                    }) as usize) as u8,
                tucse: 0,
                //set later by setter
                paylen_dtyp_tucmd: 0,
                //set later by setter
                sta_rsv: 0,
                hdrlen: 0,
                mss: 0,
            };

            desc.set_sta(0);
            desc.set_tucmd(
                // DEXT | IP | TCP
                1 << 5 | (if is_ip { 1 << 1 } else { 0 } | if is_tcp { 1 } else { 0 }) as u8,
            );

            desc.set_dtyp(0x0000);
            desc.set_paylen(0);

            self.write_bufs(
                &[desc],
                (TX_DESCS_OFF + cur_tx_desc as usize * core::mem::size_of::<TxDesc>()) as goff,
            );
            cur_tx_desc = inc_rb(cur_tx_desc, TX_BUF_COUNT as u32);

            self.txd_context_proto = txo_proto;
        }

        // Send packet
        let offset = TX_BUF_OFF + cur_tx_buf as usize * TX_BUF_SIZE;
        self.write_bufs(packet, offset as goff);

        log!(
            crate::LOG_NIC,
            "TX {} : {}..{}, {}",
            cur_tx_desc,
            offset,
            (offset + packet.len()),
            if is_udp {
                "UDP"
            }
            else {
                if is_tcp {
                    "TCP"
                }
                else {
                    if is_ip { "IP" } else { "Unknown ethertype" }
                }
            }
        );

        let mut desc = TxDataDesc {
            buffer: offset as u64,
            length_dtyp_dcmd: 0, //set later via setter
            sta_rsv: 0,          //set later as well
            popts: (((is_tcp | is_udp) as u8) << 1 | (is_ip as u8) << 0), // TXSM | IXSM
            special: 0,
        };

        desc.set_length(packet.len() as u32);
        desc.set_dtyp(0x0001);
        // DEXT | TX_CMD_EOP | TX_CMD_IFCS
        desc.set_dcmd(1 << 5 | (e1000::TX::CMD_EOP | e1000::TX::CMD_IFCS).bits());
        desc.set_sta(0);
        desc.set_rsv(0);

        self.write_bufs(
            &[desc],
            (TX_DESCS_OFF + cur_tx_desc as usize * core::mem::size_of::<TxDesc>()) as goff,
        );

        self.write_reg(e1000::REG::TDT, self.cur_tx_desc);

        true
    }

    ///Receives a single package with the max size for E1000::mtu().
    fn receive(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        //if there is nothing to receive, return
        if !self.check_irq() {
            log!(crate::LOG_NIC, "No irq");
            return Err(Error::new(Code::NotSup));
        }
        else {
            log!(crate::LOG_NIC, "Found irq");
        }

        // TODO: Improve, do it without reading registers, like quoted in the manual and how the
        // linux e1000 driver does it: "Software can determine if a receive buffer is valid by
        // reading descriptors in memory rather than by I/O reads. Any descriptor with a non-zero
        // status byte has been processed by the hardware, and is ready to be handled by the
        // software."

        // calculate field offsets. needs to happen in const to not instantiate `Buffers`.
        const RX_DESCS_OFF: usize = offset_of!(Buffers, rx_descs);

        let tail: u32 = inc_rb(self.read_reg(e1000::REG::RDT), RX_BUF_COUNT as u32);

        //Need to create the slice here, since we want to read the value after `read` took the slice
        let mut desc = [RxDesc::default()];
        self.read_bufs(
            &mut desc,
            (RX_DESCS_OFF + tail as usize * core::mem::size_of::<RxDesc>()) as goff,
        );
        let mut desc = &mut desc[0];
        // TODO: Ensure that packets that are not processed because the maxReceiveCount has been exceeded,
        // to be processed later, independently of an interrupt.

        if (desc.status & e1000::RXDS::DD.bits()) == 0 {
            return Err(Error::new(Code::NotFound));
        }

        log!(
            crate::LOG_NIC,
            "RX {}: {:x}..{:x} st={:x} er={:x}",
            tail,
            desc.buffer,
            desc.buffer + desc.length as u64,
            desc.status,
            desc.error,
        );

        //TODO in C++ the valid_checksum is uninitialized and probably not set when checked for the first
        // time. In rust we init to false, but that might produce a different result.
        let mut valid_checksum = false;
        // Ignore Checksum Indication not set
        if (desc.status & e1000::RXDS::IXSM.bits()) == 0 {
            if (desc.status & e1000::RXDS::IPCS.bits()) > 0 {
                valid_checksum = (desc.error & e1000::RXDE::IPE.bits()) == 0;

                if !valid_checksum {
                    // TODO: Increase lwIP ip drop/chksum counters
                    log!(crate::LOG_NIC, "Dropped packet with IP checksum error.");
                }
                else if (desc.status & (e1000::RXDS::TCPCS | e1000::RXDS::UDPCS).bits()) > 0 {
                    log!(
                        crate::LOG_NIC,
                        "E1000: IXMS set, bur TCPS and UDPCS set, therefore trying alternative checksum..."
                    );

                    valid_checksum = (desc.error & e1000::RXDE::TCPE.bits()) == 0;
                    if !valid_checksum {
                        // TODO: Increase lwIP tcp/udp drop/chksum counters
                        log!(
                            crate::LOG_NIC,
                            "Dropped packet with TCP/UDP checksum error. (IXMS set, TCPCS | UDPCS set)"
                        );
                    }
                }
                else {
                    // TODO: Maybe ensure that it is really not TCP/UDP?
                    log!(
                        crate::LOG_NIC,
                        "E1000: IXMS set, but checksum does not match."
                    );
                }
            }
            else {
                // TODO: Maybe ensure that it is really not IP?
                log!(
                    crate::LOG_NIC,
                    "E1000: IXMS set, ICPCS not set, skipping checksum"
                );
                valid_checksum = true;
            }
        }
        else {
            log!(crate::LOG_NIC, "E1000: IXMS not set, skipping checksum");
        }

        //TODO this was done in a loop over sub buffer, however,
        // in rust we just allocate e big enough buffer and receive the
        //package into this buffer
        let read_size;
        if valid_checksum {
            //Create buffer with enough size, initialized to 0
            assert!(
                (desc.length as usize) < E1000::mtu(),
                "desc wanted to store buffer, bigger then mtu"
            );
            self.read_bufs(&mut buf[0..desc.length.into()], desc.buffer);
            read_size = desc.length.into();
        }
        else {
            log!(
                crate::LOG_NIC,
                "Failed to validate checksum of RxDesc in E1000"
            );
            return Err(Error::new(Code::InvChecksum));
        }

        //Write back the updated rx buffer.
        desc.length = 0;
        desc.checksum = 0;
        desc.status = 0;
        desc.error = 0;
        self.write_bufs(
            &[desc],
            (RX_DESCS_OFF + tail as usize * core::mem::size_of::<RxDesc>()) as u64,
        );

        // move to next package by updating the `tail` value on the device.
        self.write_reg(e1000::REG::RDT, tail);

        Ok(read_size)
    }

    fn read_reg(&self, reg: e1000::REG) -> u32 {
        // same as above
        let val: u32 = self
            .nic
            .read_reg(reg.bits())
            .expect("failed to read NIC register");
        log!(crate::LOG_NIC, "REG[{:x}] -> {:x}", reg.bits(), val);
        val
    }

    fn write_reg(&self, reg: e1000::REG, value: u32) {
        log!(crate::LOG_NIC, "REG[{:x}] <- {:x}", reg.bits(), value);
        // there is no reasonable way to continue if that fails -> panic
        self.nic
            .write_reg(reg.bits(), value)
            .expect("failed to write NIC register");
    }

    fn read_bufs<T>(&self, data: &mut [T], offset: goff) {
        log!(
            crate::LOG_NIC,
            "Reading BUF[{:#x} .. {:#x}]",
            offset,
            offset + data.len() as goff - 1
        );
        self.bufs
            .read(data, offset)
            .expect("read from buffers failed");
    }

    fn write_bufs<T>(&self, data: &[T], offset: goff) {
        log!(
            crate::LOG_NIC,
            "Writing BUF[{:#x} .. {:#x}]",
            offset,
            offset + data.len() as goff - 1
        );
        self.bufs
            .write(data, offset)
            .expect("write to buffers failed");
    }

    fn read_eeprom(&self, address: usize, dest: &mut [u8]) {
        self.eeprom
            .read(self, address, dest)
            .expect("failed to read from EEPROM");
    }

    fn sleep(&self, usec: u64) {
        log!(crate::LOG_NIC, "NIC sleep: {}usec", usec);
        let nanos = usec * 1000;
        m3::pes::VPE::sleep_for(nanos).expect("Failed to sleep in NIC driver");
    }

    fn read_mac(&self) -> MAC {
        let macl: u32 = self.read_reg(e1000::REG::RAL);
        let mach: u32 = self.read_reg(e1000::REG::RAH);

        let mut mac = MAC::new(
            ((macl >> 0) & 0xff) as u8,
            ((macl >> 8) & 0xff) as u8,
            ((macl >> 16) & 0xff) as u8,
            ((macl >> 24) & 0xff) as u8,
            ((mach >> 0) & 0xff) as u8,
            ((mach >> 8) & 0xff) as u8,
        );

        log!(crate::LOG_NIC, "Got MAC: {}", mac);

        //if thats valid, take it
        if mac != MAC::broadcast() && mac.value() != 0 {
            return mac;
        }

        //wasn't correct, therefore try to read from eeprom
        log!(crate::LOG_NIC, "Reading MAC from EEPROM");
        let mut bytes = [0 as u8; MAC_LEN];
        self.read_eeprom(0, &mut bytes);

        mac = MAC::new(bytes[1], bytes[0], bytes[3], bytes[2], bytes[5], bytes[4]);

        log!(crate::LOG_NIC, "Got MAC from EEPROM: {}", mac);

        mac
    }

    fn link_state_changed(&mut self) -> bool {
        if self.link_state_changed {
            self.link_state_changed = false;
            true
        }
        else {
            false
        }
    }

    fn link_is_up(&self) -> bool {
        (self.read_reg(e1000::REG::STATUS) & e1000::STATUS::LU.bits() as u32) > 0
    }

    #[inline]
    fn mtu() -> usize {
        TX_BUF_SIZE
    }

    //checks if a irq occured
    fn check_irq(&mut self) -> bool {
        let icr = self.read_reg(e1000::REG::ICR);
        log!(crate::LOG_NIC, "Status: icr={:x}", icr);
        if (icr & e1000::ICR::LSC.bits() as u32) > 0 {
            self.link_state_changed = true;
        }
        self.nic.check_for_irq()
    }
}

///Wrapper around the E1000 driver, implementing smols Device trait
pub struct E1000Device {
    dev: Rc<RefCell<E1000>>,
}

impl E1000Device {
    pub fn new() -> Result<Self, Error> {
        Ok(E1000Device {
            dev: Rc::new(RefCell::new(E1000::new()?)),
        })
    }
}

impl<'a> smoltcp::phy::Device<'a> for E1000Device {
    type RxToken = RxToken;
    type TxToken = TxToken;

    fn capabilities(&self) -> smoltcp::phy::DeviceCapabilities {
        let mut caps = smoltcp::phy::DeviceCapabilities::default();
        caps.max_transmission_unit = E1000::mtu();
        caps
    }

    fn receive(&'a mut self) -> Option<(Self::RxToken, Self::TxToken)> {
        let mut buffer = vec![0 as u8; E1000::mtu()];

        match self.dev.borrow_mut().receive(&mut buffer) {
            Ok(size) => {
                buffer.resize(size, 0);
                let rx = RxToken { buffer };
                let tx = TxToken {
                    device: self.dev.clone(),
                };
                Some((rx, tx))
            },
            Err(_) => None,
        }
    }

    fn transmit(&'a mut self) -> Option<Self::TxToken> {
        Some(TxToken {
            device: self.dev.clone(),
        })
    }
}

pub struct RxToken {
    buffer: Vec<u8>,
}

impl smoltcp::phy::RxToken for RxToken {
    fn consume<R, F>(mut self, _timestamp: Instant, f: F) -> smoltcp::Result<R>
    where
        F: FnOnce(&mut [u8]) -> smoltcp::Result<R>,
    {
        f(&mut self.buffer[..])
    }
}

pub struct TxToken {
    device: Rc<RefCell<E1000>>,
}

impl smoltcp::phy::TxToken for TxToken {
    fn consume<R, F>(self, _timestamp: Instant, len: usize, f: F) -> smoltcp::Result<R>
    where
        F: FnOnce(&mut [u8]) -> smoltcp::Result<R>,
    {
        let mut buffer = vec![0; len];
        //fill buffer with "to be send" data
        let res = f(&mut buffer)?;
        if !self.device.borrow_mut().send(&buffer[..]) {
            panic!("Could not send package");
        }
        else {
            Ok(res)
        }
    }
}
