/*
 * Copyright (C) 2021-2022 Nils Asmussen, Barkhausen Institut
 * Copyright (C) 2021, Tendsin Mende <tendsin.mende@mailbox.tu-dresden.de>
 * Copyright (C) 2017, Georg Kotheimer <georg.kotheimer@mailbox.tu-dresden.de>
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
use m3::errors::{Code, Error};
use m3::io::LogFlags;
use m3::kif::{Perm, TileISA};
use m3::log;
use m3::mem::GlobOff;
use m3::net::{log_net, NetLogEvent, MAC};
use m3::time::TimeDuration;

use memoffset::offset_of;

use pci::Device;

use super::defines::*;
use super::eeprom::EEPROM;

#[inline]
fn inc_rb(index: u32, size: u32) -> u32 {
    (index + 1) % size
}

pub struct E1000 {
    nic: Device,
    eeprom: EEPROM,
    mac: MAC,

    cur_tx_desc: u32,
    cur_tx_buf: u32,

    bufs: MemGate,
    devbufs: MemGate,

    txd_context_proto: TxoProto,

    needs_poll: bool,
}

static ZEROS: [u8; 4096] = [0; 4096];

impl E1000 {
    pub fn new() -> Result<Self, Error> {
        let nic = Device::new("nic", TileISA::NICDev)?;

        let bufs = MemGate::new(core::mem::size_of::<Buffers>(), Perm::RW)?;
        let devbufs = bufs.derive(0, core::mem::size_of::<Buffers>(), Perm::RW)?;

        let mut dev = E1000 {
            eeprom: EEPROM::new(&nic)?,
            nic,
            mac: MAC::broadcast(), // gets initialised at reset

            cur_tx_desc: 0,
            cur_tx_buf: 0,

            bufs,
            devbufs,

            txd_context_proto: TxoProto::UNSUPPORTED,

            needs_poll: false,
        };

        dev.nic.set_dma_buffer(&dev.devbufs)?;

        // clear descriptors
        let mut i = 0;
        while i < core::mem::size_of::<Buffers>() {
            dev.write_bufs(
                &ZEROS,
                (core::mem::size_of::<Buffers>() - i)
                    .min(core::mem::size_of_val(&ZEROS))
                    .min(i) as GlobOff,
            );
            i += core::mem::size_of_val(&ZEROS);
        }

        // reset card
        dev.reset();

        // enable interrupts
        dev.write_reg(REG::IMC, (ICR::LSC | ICR::RXO | ICR::RXT0).bits().into());
        dev.write_reg(REG::IMS, (ICR::LSC | ICR::RXO | ICR::RXT0).bits().into());

        Ok(dev)
    }

    pub fn needs_poll(&self) -> bool {
        self.needs_poll
    }

    fn reset(&mut self) {
        // always reset MAC. Required to reset the TX and RX rings.
        let mut ctrl: u32 = self.read_reg(REG::CTRL);
        self.write_reg(REG::CTRL, ctrl | CTL::RESET.bits());
        self.sleep(RESET_SLEEP_TIME);

        // set a sensible default configuration
        ctrl |= (CTL::SLU | CTL::ASDE).bits();
        ctrl &= (CTL::LRST | CTL::FRCSPD | CTL::FRCDPLX).bits();
        self.write_reg(REG::CTRL, ctrl);
        self.sleep(RESET_SLEEP_TIME);

        // if link is already up, do not attempt to reset the PHY.  On
        // some models (notably ICH), performing a PHY reset seems to
        // drop the link speed to 10Mbps.
        let status: u32 = self.read_reg(REG::STATUS);
        if ((!status) & STATUS::LU.bits() as u32) > 0 {
            // reset PHY and MAC simultaneously
            self.write_reg(REG::CTRL, ctrl | (CTL::RESET | CTL::PHY_RESET).bits());
            self.sleep(RESET_SLEEP_TIME);

            // PHY reset is not self-clearing on all models
            self.write_reg(REG::CTRL, ctrl);
            self.sleep(RESET_SLEEP_TIME);
        }

        // enable ip/udp/tcp receive checksum offloading
        self.write_reg(REG::RXCSUM, (RXCSUM::IPOFLD | RXCSUM::TUOFLD).bits().into());

        // calculate field offsets. needs to happen in const to not instantiate `Buffers`.
        const RX_BUF_OFF: usize = offset_of!(Buffers, rx_buf);
        const TX_DESCS_OFF: usize = offset_of!(Buffers, tx_descs);
        const RX_DESCS_OFF: usize = offset_of!(Buffers, rx_descs);

        // setup rx descriptors
        for i in 0..RX_BUF_COUNT {
            // write RxDesc to descriptors
            let desc = [RxDesc {
                buffer: (RX_BUF_OFF + i * RX_BUF_SIZE) as u64,
                length: RX_BUF_SIZE as u16,
                checksum: 0,
                status: 0,
                error: 0,
                pad: 0,
            }];
            self.write_bufs(
                &desc,
                (RX_DESCS_OFF + i * core::mem::size_of::<RxDesc>()) as GlobOff,
            );

            // read it back; TODO why is that necessary?
            let mut desc = [RxDesc {
                buffer: 0,
                length: 0,
                checksum: 0,
                status: 0,
                error: 0,
                pad: 0,
            }];
            self.read_bufs(
                &mut desc,
                (RX_DESCS_OFF + i * core::mem::size_of::<RxDesc>()) as GlobOff,
            );
        }

        // init receive ring
        self.write_reg(REG::RDBAH, 0);
        self.write_reg(REG::RDBAL, RX_DESCS_OFF as u32);
        self.write_reg(
            REG::RDLEN,
            (RX_BUF_COUNT * core::mem::size_of::<RxDesc>()) as u32,
        );
        self.write_reg(REG::RDH, 0);
        self.write_reg(REG::RDT, (RX_BUF_COUNT - 1) as u32);
        self.write_reg(REG::RDTR, 0);
        self.write_reg(REG::RADV, 0);

        // init transmit ring
        self.write_reg(REG::TDBAH, 0);
        self.write_reg(REG::TDBAL, TX_DESCS_OFF as u32);
        self.write_reg(
            REG::TDLEN,
            (TX_BUF_COUNT * core::mem::size_of::<TxDesc>()) as u32,
        );
        self.write_reg(REG::TDH, 0);
        self.write_reg(REG::TDT, 0);
        self.write_reg(REG::TIDV, 0);
        self.write_reg(REG::TADV, 0);

        // enable rings
        // Always enabled for this model? legacy stuff?
        // self.write_reg(REG::RDCTL, self.read_reg(REG::RDCTL) | XDCTL::ENABLE);
        // self.write_reg(REG::TDCTL, self.read_reg(REG::TDCTL) | XDCTL::ENABLE);

        // get MAC and setup MAC filter
        self.mac = self.read_mac();
        let macval: u64 = self.mac.raw();
        self.write_reg(REG::RAL, (macval & 0xFFFFFFFF) as u32);
        self.write_reg(
            REG::RAH,
            (((macval >> 32) as u32) & 0xFFFF) | RAH::VALID.bits(),
        );

        // enable transmitter
        let mut tctl: u32 = self.read_reg(REG::TCTL);
        tctl &= !((TCTL::COLT_MASK | TCTL::COLD_MASK).bits());
        tctl |= (TCTL::ENABLE | TCTL::PSP | TCTL::COLL_DIST | TCTL::COLL_TSH).bits();
        self.write_reg(REG::TCTL, tctl);

        // enable receiver
        let mut rctl: u32 = self.read_reg(REG::RCTL);
        rctl &= !((RCTL::BSIZE_MASK | RCTL::BSEX_MASK).bits());
        rctl |= (RCTL::ENABLE | RCTL::UPE | RCTL::MPE | RCTL::BAM | RCTL::BSIZE_2K | RCTL::SECRC)
            .bits();
        self.write_reg(REG::RCTL, rctl);
    }

    pub const fn mtu() -> usize {
        // gem5 limits us to TX_BUF_SIZE - 1 (2047)
        TX_BUF_SIZE - 1
    }

    pub fn send(&mut self, packet: &[u8]) -> bool {
        assert!(packet.len() <= E1000::mtu());

        let mut next_tx_desc: u32 = inc_rb(self.cur_tx_desc, TX_BUF_COUNT as u32);

        let head: u32 = self.read_reg(REG::TDH);
        // TODO: is the condition correct or off by one?
        if next_tx_desc == head {
            log!(LogFlags::NetNIC, "e1000: no free descriptors for sending");
            return false;
        }

        // check which protocol is used: ip, tcp, or udp.
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
                let hdr = (packet as *const _ as *const u8).add(core::mem::size_of::<EthHdr>())
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

        // check if the type of package has changed, in that case update the context
        let txd_context_update_required: bool = (self.txd_context_proto & txo_proto) != txo_proto;
        if txd_context_update_required {
            next_tx_desc = inc_rb(next_tx_desc, TX_BUF_COUNT as u32);
        }

        if txd_context_update_required && (next_tx_desc == head) {
            log!(
                LogFlags::NetNIC,
                "e1000: no free descriptors to update context and transmit data"
            );
            return false;
        }

        // calculate field offsets. needs to happen in const to not instantiate `Buffers`.
        const TX_BUF_OFF: usize = offset_of!(Buffers, tx_buf);
        const TX_DESCS_OFF: usize = offset_of!(Buffers, tx_descs);

        // swap tx desc
        let mut cur_tx_desc: u32 = self.cur_tx_desc;
        self.cur_tx_desc = next_tx_desc;

        let cur_tx_buf: u32 = self.cur_tx_buf;
        self.cur_tx_buf = inc_rb(self.cur_tx_buf, TX_BUF_COUNT as u32);

        // update context descriptor if necessary (different protocol)
        if txd_context_update_required {
            let mut desc = [TxContextDesc {
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
                // set later by setter
                paylen_dtyp_tucmd: 0,
                // set later by setter
                sta_rsv: 0,
                hdrlen: 0,
                mss: 0,
            }];

            desc[0].set_sta(0);
            desc[0].set_tucmd(
                // DEXT | IP | TCP
                1 << 5 | (if is_ip { 1 << 1 } else { 0 } | u8::from(is_tcp)),
            );

            desc[0].set_dtyp(0x0000);
            desc[0].set_paylen(0);

            self.write_bufs(
                &desc,
                (TX_DESCS_OFF + cur_tx_desc as usize * core::mem::size_of::<TxDesc>()) as GlobOff,
            );
            cur_tx_desc = inc_rb(cur_tx_desc, TX_BUF_COUNT as u32);

            self.txd_context_proto = txo_proto;
        }

        // send packet
        let offset = TX_BUF_OFF + cur_tx_buf as usize * TX_BUF_SIZE;
        self.write_bufs(packet, offset as GlobOff);

        log_net(NetLogEvent::SentPacket, 0, packet.len());
        log!(
            LogFlags::NetNIC,
            "e1000: TX {} : {:#x}..{:#x}, {}",
            cur_tx_desc,
            offset,
            (offset + packet.len()),
            match txo_proto {
                TxoProto::IP => "IP",
                TxoProto::UDP => "UDP",
                TxoProto::TCP => "TCP",
                _ => "??",
            },
        );

        let mut desc = [TxDataDesc {
            buffer: offset as u64,
            length_dtyp_dcmd: 0, // set later via setter
            sta_rsv: 0,          // set later as well
            popts: (((is_tcp | is_udp) as u8) << 1 | (is_ip as u8) << 0), // TXSM | IXSM
            special: 0,
        }];

        desc[0].set_length(packet.len() as u32);
        desc[0].set_dtyp(0x0001);
        // DEXT | TX_CMD_EOP | TX_CMD_IFCS
        desc[0].set_dcmd(1 << 5 | (TX::CMD_EOP | TX::CMD_IFCS).bits());
        desc[0].set_sta(0);
        desc[0].set_rsv(0);

        self.write_bufs(
            &desc,
            (TX_DESCS_OFF + cur_tx_desc as usize * core::mem::size_of::<TxDesc>()) as GlobOff,
        );

        self.write_reg(REG::TDT, self.cur_tx_desc);

        true
    }

    fn valid_checksum(desc: &RxDesc) -> bool {
        if (desc.status & RXDS::IXSM.bits()) == 0 {
            if (desc.status & RXDS::IPCS.bits()) != 0 {
                if (desc.error & RXDE::IPE.bits()) != 0 {
                    log!(LogFlags::NetNICChksum, "e1000: IP checksum error");
                    false
                }
                else if (desc.status & (RXDS::TCPCS | RXDS::UDPCS).bits()) != 0 {
                    if (desc.error & RXDE::TCPE.bits()) != 0 {
                        log!(LogFlags::NetNICChksum, "e1000: TCP/UDP checksum error");
                        false
                    }
                    else {
                        true
                    }
                }
                else {
                    // TODO: Maybe ensure that it is really not TCP/UDP?
                    log!(
                        LogFlags::NetNICChksum,
                        "e1000: IXMS set, but checksum does not match"
                    );
                    true
                }
            }
            else {
                // TODO: Maybe ensure that it is really not IP?
                log!(
                    LogFlags::NetNIC,
                    "e1000: IXMS set, IPCS not set, skipping checksum"
                );
                true
            }
        }
        else {
            log!(LogFlags::NetNIC, "e1000: IXMS not set, skipping checksum");
            true
        }
    }

    /// Receives a single package with the max size for E1000::mtu().
    pub fn receive(&mut self) -> Result<Vec<u8>, Error> {
        // always check for IRQs to ACK them, but also always check whether there are packets
        // in the ring buffer in case we received a single IRQ for multiple packets.
        self.check_irq();

        // TODO: Improve, do it without reading registers, like quoted in the manual and how the
        // linux e1000 driver does it: "Software can determine if a receive buffer is valid by
        // reading descriptors in memory rather than by I/O reads. Any descriptor with a non-zero
        // status byte has been processed by the hardware, and is ready to be handled by the
        // software."

        // calculate field offsets. needs to happen in const to not instantiate `Buffers`.
        const RX_DESCS_OFF: usize = offset_of!(Buffers, rx_descs);

        let tail: u32 = inc_rb(self.read_reg(REG::RDT), RX_BUF_COUNT as u32);

        // need to create the slice here, since we want to read the value after `read` took the slice
        let mut desc = [RxDesc::default()];
        self.read_bufs(
            &mut desc,
            (RX_DESCS_OFF + tail as usize * core::mem::size_of::<RxDesc>()) as GlobOff,
        );

        if (desc[0].status & RXDS::DD.bits()) == 0 {
            self.needs_poll = false;
            return Err(Error::new(Code::NotFound));
        }

        log_net(NetLogEvent::RecvPacket, 0, desc[0].length as usize);
        log!(
            LogFlags::NetNIC,
            "e1000: RX {}: {:#x}..{:#x} st={:#x} er={:#x}",
            tail,
            desc[0].buffer,
            desc[0].buffer + desc[0].length as u64,
            desc[0].status,
            desc[0].error,
        );

        if !Self::valid_checksum(&desc[0]) {
            return Err(Error::new(Code::InvChecksum));
        }

        assert!((desc[0].length as usize) <= E1000::mtu());

        let read_size = desc[0].length.into();
        let mut buf = Vec::<u8>::with_capacity(read_size);
        // we deliberately use uninitialize memory here, because it's performance critical
        // safety: this is okay, because the TCU does not read from `buf`
        #[allow(clippy::uninit_vec)]
        unsafe {
            buf.set_len(read_size);
        }
        self.read_bufs(&mut buf, desc[0].buffer);

        // Write back the updated rx buffer.
        desc[0].length = 0;
        desc[0].checksum = 0;
        desc[0].status = 0;
        desc[0].error = 0;
        self.write_bufs(
            &desc,
            (RX_DESCS_OFF + tail as usize * core::mem::size_of::<RxDesc>()) as u64,
        );

        // move to next package by updating the `tail` value on the device.
        self.write_reg(REG::RDT, tail);

        // check if there is another packet and remind ourself to call receive again
        let tail: u32 = inc_rb(tail, RX_BUF_COUNT as u32);
        self.read_bufs(
            &mut desc,
            (RX_DESCS_OFF + tail as usize * core::mem::size_of::<RxDesc>()) as GlobOff,
        );
        self.needs_poll = (desc[0].status & RXDS::DD.bits()) != 0;

        Ok(buf)
    }

    pub fn read_reg(&self, reg: REG) -> u32 {
        // there is no reasonable way to continue if that fails -> panic
        let val: u32 = self
            .nic
            .read_reg(reg.into())
            .expect("failed to read NIC register");
        log!(LogFlags::NetNICDbg, "e1000: REG[{:?}] -> {:#x}", reg, val);
        val
    }

    pub fn write_reg(&self, reg: REG, value: u32) {
        log!(LogFlags::NetNICDbg, "e1000: REG[{:?}] <- {:#x}", reg, value);
        // there is no reasonable way to continue if that fails -> panic
        self.nic
            .write_reg(reg.into(), value)
            .expect("failed to write NIC register");
    }

    fn read_bufs<T>(&self, data: &mut [T], offset: GlobOff) {
        log!(
            LogFlags::NetNICDbg,
            "e1000: reading BUF[{:#x} .. {:#x}]",
            offset,
            offset + data.len() as GlobOff - 1
        );
        self.bufs
            .read(data, offset)
            .expect("read from buffers failed");
    }

    fn write_bufs<T>(&self, data: &[T], offset: GlobOff) {
        log!(
            LogFlags::NetNICDbg,
            "e1000: writing BUF[{:#x} .. {:#x}]",
            offset,
            offset + data.len() as GlobOff - 1
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

    fn sleep(&self, duration: TimeDuration) {
        log!(LogFlags::NetNIC, "e1000: sleep for {:?}", duration);
        m3::tiles::OwnActivity::sleep_for(duration).expect("Failed to sleep in NIC driver");
    }

    fn read_mac(&self) -> MAC {
        let macl: u32 = self.read_reg(REG::RAL);
        let mach: u32 = self.read_reg(REG::RAH);

        let mut mac = MAC::new(
            ((macl >> 0) & 0xff) as u8,
            ((macl >> 8) & 0xff) as u8,
            ((macl >> 16) & 0xff) as u8,
            ((macl >> 24) & 0xff) as u8,
            ((mach >> 0) & 0xff) as u8,
            ((mach >> 8) & 0xff) as u8,
        );

        log!(LogFlags::NetNIC, "e1000: got MAC: {}", mac);

        // if thats valid, take it
        if mac != MAC::broadcast() && mac.raw() != 0 {
            return mac;
        }

        // wasn't correct, therefore try to read from eeprom
        let mut bytes = [0u8; 6];
        self.read_eeprom(0, &mut bytes);

        mac = MAC::new(bytes[1], bytes[0], bytes[3], bytes[2], bytes[5], bytes[4]);

        log!(LogFlags::NetNIC, "e1000: got MAC from EEPROM: {}", mac);

        mac
    }

    fn check_irq(&mut self) -> bool {
        // the NIC does not generate another interrupt until ICR is read
        let _icr = self.read_reg(REG::ICR);
        self.nic.check_for_irq()
    }
}
