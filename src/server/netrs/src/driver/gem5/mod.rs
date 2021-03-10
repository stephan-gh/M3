use m3::cell::RefCell;
use m3::com::MemGate;
use m3::errors::{Code, Error};
use m3::goff;
use m3::kif::{Perm, PEISA};
use m3::net::{MAC, MAC_LEN};
use m3::rc::Rc;
use m3::vec::Vec;
use pci::Device;

use smoltcp::time::Instant;

pub mod defines;
use defines::*;


use memoffset::offset_of;

#[inline]
fn inc_rb(index: u32, size: u32) -> u32 {
    (index + 1) % size
}

struct EEPROM {
    shift: i32,
    done_bit: u32,
}

impl EEPROM {
    fn init(&mut self, device: &Device) -> bool {
        device.write_reg(E1000_REG::EERD.bits(), E1000_EERD::START.bits() as u32);

        let mut t = base::tcu::TCU::nanotime();
        let mut tried_once = false;
        while !tried_once && (base::tcu::TCU::nanotime() - t) < MAX_WAIT_NANOS {
            let value: u32 = device
                .read_reg(E1000_REG::EERD.bits())
                .expect("Failed to read eerd register");
            if (value & E1000_EERD::DONE_LARGE.bits() as u32) > 0 {
                log!(crate::LOG_NIC, "Detected large EERD");
                self.done_bit = E1000_EERD::DONE_LARGE.bits().into();
                self.shift = E1000_EERD::SHIFT_LARGE.bits().into();
                return true;
            }
            if (value & E1000_EERD::DONE_SMALL.bits() as u32) > 0 {
                log!(crate::LOG_NIC, "Detected small EERD");
                self.done_bit = E1000_EERD::DONE_SMALL.bits().into();
                self.shift = E1000_EERD::SHIFT_SMALL.bits().into();
                return true;
            }
            tried_once = true;
        }

        false
    }

    //reads `data` of `len` from the device.
    //TOD: Currently doing stuff with the ptr of data. Should probably give sub slices of the length of one
    // word tp the read_word fct. Also `len` is not needed since rust slice know their length.
    fn read(&self, dev: &E1000, mut address: usize, mut data: &mut [u8]) -> bool {
        assert!((data.len() & ((1 << WORD_LEN_LOG2) - 1)) == 0);

        let num_bytes_to_move = 1 << WORD_LEN_LOG2;
        let mut len = data.len();
        while len > 0 {
            if !self.read_word(dev, address, data) {
                return false;
            }
            //move to next word
            data = &mut data[num_bytes_to_move..];
            address += 1;
            len -= num_bytes_to_move;
        }
        true
    }

    fn read_word(&self, dev: &E1000, address: usize, data: &mut [u8]) -> bool {
        //cast to 16bit array
        let data_word: &mut [u16] = unsafe { core::mem::transmute::<&mut [u8], &mut [u16]>(data) };

        //set address
        dev.write_reg(
            E1000_REG::EERD,
            E1000_EERD::START.bits() as u32 | (address << self.shift) as u32,
        );

        //Wait for read to complete
        let mut t = base::tcu::TCU::nanotime();
        let mut done_once = false;
        while (base::tcu::TCU::nanotime() - t) < MAX_WAIT_NANOS && !done_once {
            let value = dev.read_reg(E1000_REG::EERD);
            done_once = true;
            if (!value & self.done_bit) != 0 {
                //Not read yet, therefore try again
                continue;
            }
            //Move word into slice
            data_word[0] = (value >> 16) as u16;
            return true;
        }
        false
    }
}

struct E1000 {
    nic: Device,
    eeprom: EEPROM,
    mac: MAC,

    cur_rx_buf: u32,
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
        let eeprom = EEPROM {
            shift: 0,
            done_bit: 0,
        };

        //TODO create global memory in rust, should be global...?
        let bufs = MemGate::new(core::mem::size_of::<Buffers>(), Perm::RW)?;
        let devbufs = bufs.derive(0, core::mem::size_of::<Buffers>(), Perm::RW)?;

        let mut dev = E1000 {
            nic,
            eeprom,
            mac: MAC::broadcast(), //gets initialised at reset

            cur_rx_buf: 0,
            cur_tx_desc: 0,
            cur_tx_buf: 0,

            bufs,
            devbufs,

            link_state_changed: true,
            txd_context_proto: TxoProto::Unsupported,
        };

        if !dev.eeprom.init(&dev.nic) {
            log!(crate::LOG_DEF, "Failed to init EEPROM");
        }

        dev.nic.set_dma_buffer(&dev.devbufs);

        //clear descriptors
        let mut i = 0;
        while i < core::mem::size_of::<Buffers>() {
            dev.bufs.write(
                &ZEROS,
                (core::mem::size_of::<Buffers>() - i)
                    .min(core::mem::size_of_val(&ZEROS))
                    .min(i) as u64,
            );
            i += core::mem::size_of_val(&ZEROS);
        }

        //Reset card
        dev.reset();

        //Enable interrupts
        dev.write_reg(
            E1000_REG::IMC,
            (E1000_ICR::LSC | E1000_ICR::RXO | E1000_ICR::RXT0)
                .bits()
                .into(),
        );
        dev.write_reg(
            E1000_REG::IMS,
            (E1000_ICR::LSC | E1000_ICR::RXO | E1000_ICR::RXT0)
                .bits()
                .into(),
        );

        Ok(dev)
    }

    /*TODO not needed for rust pci device?
    fn stop(&self){
    self.nic.stop_listening();
    }
     */

    fn reset(&mut self) {
        //always reset MAC. Required to reset the TX and RX rings.
        let mut ctrl: u32 = self.read_reg(E1000_REG::CTRL);
        self.write_reg(E1000_REG::CTRL, (ctrl | E1000_CTL::RESET.bits()));
        self.sleep(RESET_SLEEP_TIME);

        //set a sensible default configuration
        ctrl |= (E1000_CTL::SLU | E1000_CTL::ASDE).bits();
        ctrl &= (E1000_CTL::LRST | E1000_CTL::FRCSPD | E1000_CTL::FRCDPLX).bits();
        self.write_reg(E1000_REG::CTRL, ctrl);
        self.sleep(RESET_SLEEP_TIME);

        // if link is already up, do not attempt to reset the PHY.  On
        // some models (notably ICH), performing a PHY reset seems to
        // drop the link speed to 10Mbps.
        let status: u32 = self.read_reg(E1000_REG::STATUS);
        if ((!status) & E1000_STATUS::LU.bits() as u32) > 0 {
            // Reset PHY and MAC simultaneously
            self.write_reg(
                E1000_REG::CTRL,
                ctrl | (E1000_CTL::RESET | E1000_CTL::PHY_RESET).bits(),
            );
            self.sleep(RESET_SLEEP_TIME);

            // PHY reset is not self-clearing on all models
            self.write_reg(E1000_REG::CTRL, ctrl);
            self.sleep(RESET_SLEEP_TIME);
        }

        // enable ip/udp/tcp receive checksum offloading
        self.write_reg(
            E1000_REG::RXCSUM,
            (E1000_RXCSUM::IPOFLD | E1000_RXCSUM::TUOFLD).bits().into(),
        );

        // setup rx descriptors
        for i in 0..RX_BUF_COUNT {
            //Init rxdesc which is written
            let mut desc = RxDesc {
                buffer: (offset_of!(Buffers, rx_buf) + i * RX_BUF_SIZE) as u64,
                length: RX_BUF_SIZE as u16,
                checksum: 0,
                status: 0,
                error: 0,
                pad: 0,
            };
            log!(crate::LOG_NIC, "{} w {}", i, desc.buffer);
            self.bufs.write(
                &[desc],
                (offset_of!(Buffers, rx_descs) + i * core::mem::size_of::<RxDesc>()) as u64,
            );

            let mut desc2 = [RxDesc {
                buffer: 0,
                length: 0,
                checksum: 0,
                status: 0,
                error: 0,
                pad: 0,
            }];

            self.bufs.read(
                &mut desc2,
                (offset_of!(Buffers, rx_descs) + i * core::mem::size_of::<RxDesc>()) as u64,
            );
            log!(crate::LOG_NIC, "{} r {}", i, desc2[0].buffer);
        }

        // init receive ring
        self.write_reg(E1000_REG::RDBAH, 0);
        self.write_reg(E1000_REG::RDBAL, offset_of!(Buffers, rx_descs) as u32);
        self.write_reg(
            E1000_REG::RDLEN,
            (RX_BUF_COUNT * core::mem::size_of::<RxDesc>()) as u32,
        );
        self.write_reg(E1000_REG::RDH, 0);
        self.write_reg(E1000_REG::RDT, (RX_BUF_COUNT - 1) as u32);
        self.write_reg(E1000_REG::RDTR, 0);
        self.write_reg(E1000_REG::RADV, 0);

        // init transmit ring
        self.write_reg(E1000_REG::TDBAH, 0);
        self.write_reg(E1000_REG::TDBAL, offset_of!(Buffers, tx_descs) as u32);
        self.write_reg(
            E1000_REG::TDLEN,
            (TX_BUF_COUNT * core::mem::size_of::<TxDesc>()) as u32,
        );
        self.write_reg(E1000_REG::TDH, 0);
        self.write_reg(E1000_REG::TDT, 0);
        self.write_reg(E1000_REG::TIDV, 0);
        self.write_reg(E1000_REG::TADV, 0);

        // enable rings
        // Always enabled for this model? legacy stuff?
        // writeReg(REG_RDCTL, readReg(REG_RDCTL) | XDCTL_ENABLE);
        // writeReg(REG_TDCTL, readReg(REG_TDCTL) | XDCTL_ENABLE);

        // get MAC and setup MAC filter
        self.mac = self.read_mac();
        let macval: u64 = self.mac.value();
        self.write_reg(E1000_REG::RAL, (macval & 0xFFFFFFFF) as u32);
        self.write_reg(
            E1000_REG::RAH,
            (((macval >> 32) as u32) & 0xFFFF) | (E1000_RAH::VALID.bits() as u32),
        );

        // enable transmitter
        let mut tctl: u32 = self.read_reg(E1000_REG::TCTL);
        tctl &= (!((E1000_TCTL::COLT_MASK | E1000_TCTL::COLD_MASK).bits() as u32));
        tctl |=
            (E1000_TCTL::ENABLE | E1000_TCTL::PSP | E1000_TCTL::COLL_DIST | E1000_TCTL::COLL_TSH)
                .bits() as u32;
        self.write_reg(E1000_REG::TCTL, tctl);

        // enable receiver
        let mut rctl: u32 = self.read_reg(E1000_REG::RCTL);
        rctl &= !((E1000_RCTL::BSIZE_MASK | E1000_RCTL::BSEX_MASK).bits() as u32);
        rctl |= (E1000_RCTL::ENABLE
            | E1000_RCTL::UPE
            | E1000_RCTL::MPE
            | E1000_RCTL::BAM
            | E1000_RCTL::BSIZE_2K
            | E1000_RCTL::SECRC)
            .bits() as u32;
        self.write_reg(E1000_REG::RCTL, rctl);

        self.link_state_changed = true;
    }

    fn send(&mut self, packet: &[u8]) -> bool {
        assert!(
            packet.len() < E1000::mtu(),
            "Package was too big for E1000 device"
        );

        let mut next_tx_desc: u32 = inc_rb(self.cur_tx_desc, TX_BUF_COUNT as u32);

        let head: u32 = self.read_reg(E1000_REG::TDH);
        // TODO: Is the condition correct or off by one?
        if (next_tx_desc == head) {
            log!(crate::LOG_NIC, "No free descriptors.");
            return false;
        }

        //Check which protocol is used, ip, tcp, udp.
        let (is_ip, mut txo_proto) = {
            unsafe {
                //println!("ETH frame: {}", *(packet as *const _ as *const EthHdr));
            }

            let ethf = smoltcp::wire::EthernetFrame::new_unchecked(packet);
            if ethf.ethertype() == smoltcp::wire::EthernetProtocol::Ipv4 {
                (true, TxoProto::IP)
            }
            else {
                (false, TxoProto::Unsupported)
            }
        };

        //println!("Found is_ip={}, txo_proto={:x}", is_ip, txo_proto);
        //let mut txo_proto = if is_ip {TxoProto::IP} else {TxoProto::Unsupported};
        if ((txo_proto == TxoProto::IP)
            && ((core::mem::size_of::<EthHdr>() + core::mem::size_of::<IpHdr>()) < packet.len()))
        {
            //println!("Searched for: tcp={}, udp={}", TxoProto::TCP.bits(), TxoProto::UDP.bits());
            let proto: u8 = unsafe {
                let hdr = ((packet as *const _ as *const u8)
                    .offset(core::mem::size_of::<EthHdr>() as isize)
                    as *const IpHdr);
                //println!("hdr: {}", *hdr);
                (*hdr).proto
            };
            if (proto == IP_PROTO_TCP) {
                txo_proto = TxoProto::TCP;
            }
            else if (proto == IP_PROTO_UDP) {
                txo_proto = TxoProto::UDP;
            }

            //let v_hl: u8 = unsafe{
            //    (*((packet as *const _ as *const u8).offset(core::mem::size_of::<EthHdr>() as isize) as *const IpHdr)).v_hl
            //};
            // lwIP uses no IP options, unless IGMP is enabled.
            //TODO not using lwIP, but smoltcp, is assert valid?
            //assert!((v_hl & 0xf) == 5); // 5 in big endian
        };

        let is_tcp = txo_proto == TxoProto::TCP;
        let is_udp = txo_proto == TxoProto::UDP;

        //check if the type of package has changed, in that case update the context
        let txd_context_update_required: bool = self.txd_context_proto != txo_proto;
        let incremented_next_tx_desc = inc_rb(next_tx_desc, TX_BUF_COUNT as u32);

        if txd_context_update_required && (incremented_next_tx_desc == head) {
            log!(
                crate::LOG_NIC,
                "Not enough free descriptors to update context and transmit data."
            );
            return false;
        }

        //swap tx desc
        let mut cur_tx_desc: u32 = self.cur_tx_desc;
        self.cur_tx_desc = next_tx_desc;

        let cur_tx_buf: u32 = self.cur_tx_buf;
        self.cur_tx_buf = inc_rb(self.cur_tx_buf, TX_BUF_COUNT as u32);

        // Update context descriptor if necessary (different protocol)
        if (txd_context_update_required) {
            log!(crate::LOG_NIC, "Writing context descriptor.");

            let mut desc = TxContextDesc {
                IPCSS: 0,
                IPCSO: (core::mem::size_of::<EthHdr>() + offset_of!(IpHdr, chksum)) as u8,
                IPCSE: 0,
                TUCSS: 0,
                TUCSO: (core::mem::size_of::<EthHdr>()
                    + core::mem::size_of::<IpHdr>()
                    + (if is_tcp {
                        TCP_CHECKSUM_OFFSET
                    }
                    else {
                        UDP_CHECKSUM_OFFSET
                    }) as usize) as u8,
                TUCSE: 0,
                //set later by setter
                PAYLEN_DTYP_TUCMD: 0,
                //set later by setter
                STA_RSV: 0,
                HDRLEN: 0,
                MSS: 0,
            };

            desc.set_sta(0);
            desc.set_tucmd(
                1 << 5 | (if is_ip { 1 << 1 } else { 0 } | if is_tcp { 1 } else { 0 }) as u8,
            ); // DEXT | IP | TCP

            desc.set_dtyp(0x0000);
            desc.set_paylen(0);

            self.bufs.write(
                &[desc],
                (offset_of!(Buffers, tx_descs)
                    + cur_tx_desc as usize * core::mem::size_of::<TxDesc>()) as u64,
            );
            cur_tx_desc = inc_rb(cur_tx_desc, TX_BUF_COUNT as u32);

            self.txd_context_proto = txo_proto;
        }

        // Send packet
        let offset = offset_of!(Buffers, tx_buf) + cur_tx_buf as usize * TX_BUF_SIZE;
        self.bufs.write(packet, offset as u64);

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
                    if is_ip {
                        "IP"
                    }
                    else {
                        "Unknown ethertype"
                    }
                }
            }
        );

        let mut desc = TxDataDesc {
            buffer: offset as u64,
            length_DTYP_DCMD: 0, //set later via setter
            STA_RSV: 0,          //set later as well
            POPTS: (((is_tcp | is_udp) as u8) << 1 | (is_ip as u8) << 0), // TXSM | IXSM
            special: 0,
        };

        desc.set_length(packet.len() as u32);
        desc.set_dtyp(0x0001);
        desc.set_dcmd(1 << 5 | (E1000_TX::CMD_EOP | E1000_TX::CMD_IFCS).bits()); // DEXT | TX_CMD_EOP | TX_CMD_IFCS
        desc.set_sta(0);
        desc.set_rsv(0);

        /*
        let cast_desc: &[u64] = unsafe{core::slice::from_raw_parts(&desc as *const _ as *const u64, 2)};
        log!(crate::LOG_NIC, "TxdataDesc= {} {}", cast_desc[0], cast_desc[1]);


        let loc = (offset_of!(Buffers, tx_descs) + cur_tx_desc as usize * core::mem::size_of::<TxDesc>()) as u64;
        log!(crate::LOG_NIC, "buffer location to write to={}", loc);
        */
        self.bufs.write(
            &[desc],
            (offset_of!(Buffers, tx_descs) + cur_tx_desc as usize * core::mem::size_of::<TxDesc>())
                as u64,
        );

        self.write_reg(E1000_REG::TDT, self.cur_tx_desc);

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

        // TODO: Improve, do it without reading registers, like quoted in the manual and how the linux e1000 driver does it.
        // " Software can determine if a receive buffer is valid by reading descriptors in memory
        //   rather than by I/O reads. Any descriptor with a non-zero status byte has been processed by the
        //   hardware, and is ready to be handled by the software."

        let tail: u32 = inc_rb(self.read_reg(E1000_REG::RDT), RX_BUF_COUNT as u32);

        //Need to create the slice here, since we want to read the value after `read` took the slice
        let mut desc = [RxDesc::default()];
        self.bufs.read(
            &mut desc,
            (offset_of!(Buffers, rx_descs) + tail as usize * core::mem::size_of::<RxDesc>()) as u64,
        );
        let mut desc = &mut desc[0];
        // TODO: Ensure that packets that are not processed because the maxReceiveCount has been exceeded,
        // to be processed later, independently of an interrupt.

        if (desc.status & E1000_RXDS::DD.bits()) == 0 {
            return Err(Error::new(Code::NotSup)); //TODO throw correct error
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
        if ((desc.status & E1000_RXDS::IXSM.bits()) == 0) {
            if ((desc.status & E1000_RXDS::IPCS.bits()) > 0) {
                valid_checksum = ((desc.error & E1000_RXDE::IPE.bits()) == 0);

                if !valid_checksum {
                    // TODO: Increase lwIP ip drop/chksum counters
                    log!(crate::LOG_NIC, "Dropped packet with IP checksum error.");
                }
                else if ((desc.status & (E1000_RXDS::TCPCS | E1000_RXDS::UDPCS).bits()) > 0) {
                    log!(crate::LOG_NIC, "E1000: IXMS set, bur TCPS and UDPCS set, therefore trying alternative checksum...");

                    valid_checksum = (desc.error & E1000_RXDE::TCPE.bits()) == 0;
                    if !valid_checksum {
                        // TODO: Increase lwIP tcp/udp drop/chksum counters
                        log!(crate::LOG_NIC, "Dropped packet with TCP/UDP checksum error. (IXMS set, TCPCS | UDPCS set)");
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
        let mut read_size = 0;
        if valid_checksum {
            //Create buffer with enough size, initialized to 0
            assert!(
                (desc.length as usize) < E1000::mtu(),
                "desc wanted to store buffer, bigger then mtu"
            );
            self.bufs.read(&mut buf[0..desc.length.into()], desc.buffer);
            read_size = desc.length.into();
        }
        else {
            log!(
                crate::LOG_NIC,
                "Failed to validate checksum of RxDesc in E1000"
            );
            return Err(Error::new(Code::NotSup)); //TODO return correct error
        }

        //Write back the updated rx buffer.
        desc.length = 0;
        desc.checksum = 0;
        desc.status = 0;
        desc.error = 0;
        self.bufs.write(
            &[desc],
            (offset_of!(Buffers, rx_descs) + tail as usize * core::mem::size_of::<RxDesc>()) as u64,
        );

        // move to next package by updating the `tail` value on the device.
        self.write_reg(E1000_REG::RDT, tail);
        //tail = inc_rb(tail, RX_BUF_COUNT as u32);
        //self.bufs.read(&mut [desc], (offset_of!(Buffers, rx_descs) + tail as usize * core::mem::size_of::<RxDesc>()) as u64);
        Ok(read_size)
    }

    fn write_reg(&self, reg: E1000_REG, value: u32) {
        log!(crate::LOG_NIC, "REG[{:x}] <- {:x}", reg.bits(), value);
        self.nic.write_reg(reg.bits(), value);
    }

    fn read_reg(&self, reg: E1000_REG) -> u32 {
        //TODO: Unwrapping since we would anyways have to panic if reading a register fails
        //Maybe retry if this fails later.
        let val: u32 = self
            .nic
            .read_reg(reg.bits())
            .expect("Failed to read NIC register");
        log!(crate::LOG_NIC, "REG[{:x}] -> {:x}", reg.bits(), val);
        val
    }

    fn read_eeprom(&self, address: usize, dest: &mut [u8]) {
        if !self.eeprom.read(self, address, dest) {
            log!(crate::LOG_NIC, "Failed to read from eeprom");
        }
    }

    fn sleep(&self, usec: u64) {
        log!(crate::LOG_NIC, "NIC sleep: {}usec", usec);
        let nanos = usec * 1000;
        let t = base::tcu::TCU::nanotime();
        m3::tcu::TCUIf::sleep_for(nanos).expect("Failed to sleep in NIC driver");
    }

    fn read_mac(&self) -> MAC {
        let macl: u32 = self.read_reg(E1000_REG::RAL);
        let mach: u32 = self.read_reg(E1000_REG::RAH);

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
        (self.read_reg(E1000_REG::STATUS) & E1000_STATUS::LU.bits() as u32) > 0
    }

    #[inline]
    fn mtu() -> usize {
        TX_BUF_SIZE
    }

    //checks if a irq occured
    fn check_irq(&mut self) -> bool {
        let icr = self.read_reg(E1000_REG::ICR);
        log!(crate::LOG_NIC, "Status: icr={:x}", icr);
        if (icr & E1000_ICR::LSC.bits() as u32) > 0 {
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
            Err(e) => None,
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
