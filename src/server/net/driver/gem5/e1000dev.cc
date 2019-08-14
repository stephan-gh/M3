/*
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

#include "../proto_def.h"
#include "e1000dev.h"

#include <assert.h>
#include <base/DTU.h>
#include <base/log/Services.h>

#include <cstddef>
#include <cstdlib>

/* parts of the code are inspired by the Escape e1000 driver */

using namespace m3;

namespace net {

static uint8_t ZEROS[4096];

static inline uint32_t incRb(uint32_t index, uint32_t size)
{
    return (index + 1) % size;
}

NetDriver *NetDriver::create(const char *, m3::WorkLoop *wl, alloc_cb_func allocCallback,
                             next_buf_cb_func nextBufCallback, recv_cb_func recvCallback) {
    return new E1000(wl, allocCallback, nextBufCallback, recvCallback);
}

E1000::E1000(WorkLoop *wl, alloc_cb_func allocCallback, next_buf_cb_func nextBufCallback,
             recv_cb_func recvCallback)
    : _nic("nic", m3::PEISA::NIC),
      _eeprom(*this),
      _curRxBuf(0),
      _curTxDesc(0),
      _curTxBuf(0),
      _bufs(MemGate::create_global(sizeof(Buffers), MemGate::RW)),
      _allocCallback(allocCallback),
      _nextBufCallback(nextBufCallback),
      _recvCallback(recvCallback),
      _linkStateChanged(true),
      _txdContextProto(TxoProto_Unsupported) {

    if(!_eeprom.init()) {
        SLOG(NIC, "Unable to init EEPROM.");
    }

    // configure dma endpoint to access descriptor buffers
    _nic.setDmaEp(_bufs);

    // register interrupt callback
    _nic.listenForIRQs(wl, std::bind(&E1000::receiveInterrupt, this));

    // clear descriptors
    for(size_t i = 0; i < sizeof(Buffers); i += sizeof(ZEROS))
        _bufs.write(&ZEROS, std::min(sizeof(Buffers) - i, sizeof(ZEROS)), i);

    // reset card
    reset();

    // enable interrupts
    writeReg(REG_IMC, ICR_LSC | ICR_RXO | ICR_RXT0);
    writeReg(REG_IMS, ICR_LSC | ICR_RXO | ICR_RXT0);
}

void E1000::stop() {
    _nic.stopListing();
}

void E1000::reset() {
    // always reset MAC.  Required to reset the TX and RX rings.
    uint32_t ctrl = readReg(REG_CTRL);
    writeReg(REG_CTRL, ctrl | CTL_RESET);
    sleep(RESET_SLEEP_TIME);

    // set a sensible default configuration
    ctrl |= CTL_SLU | CTL_ASDE;
    ctrl &= ~static_cast<uint32_t>(CTL_LRST | CTL_FRCSPD | CTL_FRCDPLX);
    writeReg(REG_CTRL, ctrl);
    sleep(RESET_SLEEP_TIME);

    // if link is already up, do not attempt to reset the PHY.  On
    // some models (notably ICH), performing a PHY reset seems to
    // drop the link speed to 10Mbps.
    uint32_t status = readReg(REG_STATUS);
    if(~status & STATUS_LU) {
        // Reset PHY and MAC simultaneously
        writeReg(REG_CTRL, ctrl | static_cast<uint32_t>(CTL_RESET | CTL_PHY_RESET));
        sleep(RESET_SLEEP_TIME);

        // PHY reset is not self-clearing on all models
        writeReg(REG_CTRL, ctrl);
        sleep(RESET_SLEEP_TIME);
    }

    // enable ip/udp/tcp receive checksum offloading
    writeReg(REG_RXCSUM, RXCSUM_IPOFLD | RXCSUM_TUOFLD);

    // setup rx descriptors
    for(size_t i = 0; i < RX_BUF_COUNT; i++) {
        RxDesc desc = {};
        desc.length = RX_BUF_SIZE;
        desc.buffer = offsetof(Buffers, rxBuf) + i * RX_BUF_SIZE;
        SLOG(NIC, i << " w " << desc.buffer);
        _bufs.write(&desc, sizeof(RxDesc), offsetof(Buffers, rxDescs) + i * sizeof(RxDesc));
        RxDesc desc2= {};
        _bufs.read(&desc2, sizeof(RxDesc), offsetof(Buffers, rxDescs) + i * sizeof(RxDesc));
        SLOG(NIC, i << " r " << desc2.buffer);
    }

    // init receive ring
    writeReg(REG_RDBAH, 0);
    writeReg(REG_RDBAL, offsetof(Buffers, rxDescs));
    writeReg(REG_RDLEN, RX_BUF_COUNT * sizeof(RxDesc));
    writeReg(REG_RDH, 0);
    writeReg(REG_RDT, RX_BUF_COUNT - 1);
    writeReg(REG_RDTR, 0);
    writeReg(REG_RADV, 0);

    // init transmit ring
    writeReg(REG_TDBAH, 0);
    writeReg(REG_TDBAL, offsetof(Buffers, txDescs));
    writeReg(REG_TDLEN, TX_BUF_COUNT * sizeof(TxDesc));
    writeReg(REG_TDH, 0);
    writeReg(REG_TDT, 0);
    writeReg(REG_TIDV, 0);
    writeReg(REG_TADV, 0);

    // enable rings
    // Always enabled for this model? legacy stuff?
    // writeReg(REG_RDCTL, readReg(REG_RDCTL) | XDCTL_ENABLE);
    // writeReg(REG_TDCTL, readReg(REG_TDCTL) | XDCTL_ENABLE);

    // get MAC and setup MAC filter
    _mac = readMAC();
    uint64_t macval = _mac.value();
    writeReg(REG_RAL, macval & 0xFFFFFFFF);
    writeReg(REG_RAH, ((macval >> 32) & 0xFFFF) | static_cast<uint32_t>(RAH_VALID));

    // enable transmitter
    uint32_t tctl = readReg(REG_TCTL);
    tctl &= ~static_cast<uint32_t>(TCTL_COLT_MASK | TCTL_COLD_MASK);
    tctl |= TCTL_ENABLE | TCTL_PSP | TCTL_COLL_DIST | TCTL_COLL_TSH;
    writeReg(REG_TCTL, tctl);

    // enable receiver
    uint32_t rctl = readReg(REG_RCTL);
    rctl &= ~static_cast<uint32_t>(RCTL_BSIZE_MASK | RCTL_BSEX_MASK);
    rctl |= RCTL_ENABLE | RCTL_UPE | RCTL_MPE | RCTL_BAM | RCTL_BSIZE_2K | RCTL_SECRC;
    writeReg(REG_RCTL, rctl);

    _linkStateChanged = true;
}

void E1000::writeReg(uint16_t reg,uint32_t value) {
    SLOG(NIC, "REG[" << fmt(reg, "#0x", 4) << "] <- " << fmt(value, "#0x", 8));
    _nic.writeReg(reg, value);
}

uint32_t E1000::readReg(uint16_t reg) {
    uint32_t val = _nic.readReg<uint32_t>(reg);
    SLOG(NIC, "REG[" << fmt(reg, "#0x", 4) << "] -> " << fmt(val, "#0x", 8));
    return val;
}

void E1000::readEEPROM(uintptr_t address, uint8_t *dest, size_t len) {
    if(!_eeprom.read(address, dest, len)) {
        SLOG(NIC, "Unable to read from EEPROM.");
        return;
    }
}

void E1000::sleep(cycles_t usec) {
    auto cycles_to_seep = usec * DTU::get().clock() / 1000000;
    SLOG(NIC, "sleep: " << usec << " usec -> " << cycles_to_seep << " cycles");
    auto t = DTU::get().tsc();
    do {
        cycles_t sleep_time = (DTU::get().tsc() - t);
        if(cycles_to_seep > sleep_time)
            DTUIf::sleep_for(cycles_to_seep - sleep_time);
    } while ((DTU::get().tsc() - t) < cycles_to_seep);
}

bool E1000::send(const void* packet, size_t size) {
    assert(size <= mtu());

    uint32_t nextTxDesc = incRb(_curTxDesc, TX_BUF_COUNT);
    uint32_t head = readReg(REG_TDH);
    // TODO: Is the condition correct or off by one?
    if(nextTxDesc == head) {
        SLOG(NIC, "No free descriptors.");
        return false;
    }

    bool isIp = sizeof(eth_hdr) < size && static_cast<const eth_hdr *>(packet)->type == ETHTYPE_IP;
    TxoProto txoProto = isIp ? TxoProto_IP : TxoProto_Unsupported;
    if(txoProto == TxoProto_IP && sizeof(eth_hdr) + sizeof(ip_hdr) < size) {
        const ip_hdr * iphdr = reinterpret_cast<const ip_hdr *>(static_cast<const uint8_t *>(packet) + sizeof(eth_hdr));
        uint8_t proto =  iphdr->proto;
        if(proto == IP_PROTO_TCP) {
            txoProto = TxoProto_TCP;
        }
        else if(proto == IP_PROTO_UDP) {
            txoProto = TxoProto_UDP;
        }

        // lwIP uses no IP options, unless IGMP is enabled.
        assert((iphdr->v_hl & 0xf) == 5); // 5 in big endian
    };

    bool isTcp = txoProto == TxoProto_TCP;
    bool isUdp = txoProto == TxoProto_UDP;

    bool txdContextUpdateRequired = (_txdContextProto & txoProto) != txoProto;
    if(txdContextUpdateRequired && (nextTxDesc = incRb(nextTxDesc, TX_BUF_COUNT)) == head) {
        SLOG(NIC, "Not enough free descriptors to update context and transmit data.");
        return false;
    }

    uint32_t curTxDesc = _curTxDesc;
    _curTxDesc = nextTxDesc;

    uint32_t curTxBuf = _curTxBuf;
    _curTxBuf = incRb(_curTxBuf, TX_BUF_COUNT);

    // Update context descriptor if necessary (different protocol)
    if(txdContextUpdateRequired) {
        SLOG(NIC, "Writing context descriptor.");

        TxContextDesc desc = {};
        desc.TUCSE = 0;
        desc.TUCSO = sizeof(eth_hdr) + sizeof(ip_hdr) + (isTcp ? TCP_CHECKSUM_OFFSET : UDP_CHECKSUM_OFFSET);
        desc.TUCSS = 0;
        desc.IPCSE = 0;
        desc.IPCSO = sizeof(eth_hdr) + offsetof(ip_hdr, chksum);
        desc.IPCSS = 0;
        desc.MSS = 0;
        desc.HDRLEN = 0;
        desc.STA = 0;
        desc.TUCMD = 1 << 5 | static_cast<uint8_t>(isIp << 1 | isTcp << 0); // DEXT | IP | TCP
        desc.DTYP = 0x0000;
        desc.PAYLEN = 0;

        _bufs.write(&desc, sizeof(TxDesc), offsetof(Buffers, txDescs) + curTxDesc * sizeof(TxDesc));
        curTxDesc = incRb(curTxDesc, TX_BUF_COUNT);

        _txdContextProto = txoProto;
    }

    // Send packet
    auto offset = offsetof(Buffers, txBuf) + curTxBuf * TX_BUF_SIZE;
    _bufs.write(packet, size, offset);

    SLOG(NIC, "TX " << curTxDesc << ": " << offset << ".." << (offset + size) << ", " << (isUdp ? "UDP" : (isTcp ? "TCP" : (isIp ? "IP" : "Unknown ethertype"))));

    // setup descriptor
    TxDataDesc desc = {};
    desc.buffer = offset;
    desc.length = size;
    desc.DTYP = 0x0001;
    desc.DCMD = 1 << 5 | TX_CMD_EOP | TX_CMD_IFCS; // DEXT | TX_CMD_EOP | TX_CMD_IFCS
    desc.STA = 0;
    desc.RSV = 0;
    desc.POPTS = static_cast<uint8_t>((isTcp | isUdp) << 1 | isIp << 0); // TXSM | IXSM
    desc.Special = 0;

    _bufs.write(&desc, sizeof(TxDesc), offsetof(Buffers, txDescs) + curTxDesc * sizeof(TxDesc));

    writeReg(REG_TDT, _curTxDesc);
    SLOG(NIC, "Status" << fmt(readReg(REG_STATUS), "x", 4));

    return true;
}

void E1000::receive(size_t maxReceiveCount) {
    // TODO: Improve, do it without reading registers, like quoted in the manual and how the linux e1000 driver does it.
    // " Software can determine if a receive buffer is valid by reading descriptors in memory
    //   rather than by I/O reads. Any descriptor with a non-zero status byte has been processed by the
    //   hardware, and is ready to be handled by the software."

    uint32_t tail = incRb(readReg(REG_RDT), RX_BUF_COUNT);
    RxDesc desc;
    _bufs.read(&desc, sizeof(RxDesc), offsetof(Buffers, rxDescs) + tail * sizeof(RxDesc));
    // TODO: Ensure that packets that are not processed because the maxReceiveCount has been exceeded,
    // to be processed later, independently of an interrupt.
    while((desc.status & RXDS_DD) && maxReceiveCount-- > 0) {
        SLOG(NIC, "RX " << tail
                << ": " << fmt(desc.buffer, "#0x", 8)
                << ".." << fmt(desc.buffer + desc.length, "#0x", 8)
                << " st=" << fmt(desc.status, "#0x", 2)
                << " er=" << fmt(desc.error, "#0x", 2));

        bool validChecksum;
        // Ignore Checksum Indication not set
        if(!(desc.status & RXDS_IXSM))
        {
            if(desc.status & RXDS_IPCS)
            {
                validChecksum = !(desc.error & RXDE_IPE);
                if(!validChecksum)
                {
                    // TODO: Increase lwIP ip drop/chksum counters
                    SLOG(NIC, "Dropped packet with IP checksum error.");
                }
                else if(desc.status & (RXDS_TCPCS | RXDS_UDPCS))
                {
                    validChecksum = !(desc.error & RXDE_TCPE);
                    if(!validChecksum)
                    {
                        // TODO: Increase lwIP tcp/udp drop/chksum counters
                        SLOG(NIC, "Dropped packet with TCP/UDP checksum error.");
                    }
                }
                else
                {
                    // TODO: Maybe ensure that it is really not TCP/UDP?
                }
            }
            else
            {
                // TODO: Maybe ensure that it is really not IP?
                validChecksum = true;
            }
        }

        if(validChecksum)
        {
            // read data into packet
           size_t size = desc.length;
           void * pkt = 0;
           void * buf = 0;
           size_t bufSize = 0;
           if(!_allocCallback(pkt, buf, bufSize, size)) {
               SLOG(NIC, "Failed to allocate buffer to read packet.");
               break;
           }

           void * pkt_head = pkt;
           size_t readCount = 0;
           do {
               size_t readSize = std::min(bufSize, size - readCount);
               SLOG(NIC, "_bufs.read: " << buf  << " "  << readSize << " "  << readCount);
               _bufs.read(buf, readSize, desc.buffer + readCount);
               readCount += readSize;
               if(readCount == size)
                   break;
               _nextBufCallback(pkt, buf, bufSize);
           } while(true);


           _recvCallback(pkt_head);
        }


        desc.length = 0;
        desc.checksum = 0;
        desc.status = 0;
        desc.error = 0;
        _bufs.write(&desc, sizeof(RxDesc), offsetof(Buffers, rxDescs) + tail * sizeof(RxDesc));

        // to next packet
        writeReg(REG_RDT, tail);
        tail = incRb(tail, RX_BUF_COUNT);
        _bufs.read(&desc, sizeof(RxDesc), offsetof(Buffers, rxDescs) + tail * sizeof(RxDesc));
    }
}

void E1000::receiveInterrupt() {
    // TODO: Check Interrupt cause register...
    uint32_t icr = readReg(REG_ICR);
    SLOG(NIC, "Received interrupt: " << fmt(icr, "#0x", 8));

    if(icr & E1000::ICR_LSC) {
        _linkStateChanged = true;
    }

    receive(MAX_RECEIVE_COUNT_PER_INTERRUPT);
}

m3::net::MAC E1000::readMAC() {
    // read current address from RAL/RAH
    uint32_t macl, mach;
    macl = readReg(REG_RAL);
    mach = readReg(REG_RAH);

    m3::net::MAC macaddr(
        (macl >>  0) & 0xFF,
        (macl >>  8) & 0xFF,
        (macl >> 16) & 0xFF,
        (macl >> 24) & 0xFF,
        (mach >>  0) & 0xFF,
        (mach >>  8) & 0xFF
    );

    SLOG(NIC, "Got MAC "
        << fmt(macaddr.bytes()[0], "0x", 2) << ":" << fmt(macaddr.bytes()[1], "0x", 2) << ":"
        << fmt(macaddr.bytes()[2], "0x", 2) << ":" << fmt(macaddr.bytes()[3], "0x", 2) << ":"
        << fmt(macaddr.bytes()[4], "0x", 2) << ":" << fmt(macaddr.bytes()[5], "0x", 2) << " from RAL/RAH");

    // if thats valid, take it
    if(macaddr != m3::net::MAC::broadcast() && macaddr.value() != 0)
        return macaddr;

    SLOG(NIC, "Reading MAC from EEPROM");
    uint8_t bytes[m3::net::MAC::LEN];
    readEEPROM(0, bytes, sizeof(bytes));

    macaddr = m3::net::MAC(
        bytes[1],
        bytes[0],
        bytes[3],
        bytes[2],
        bytes[5],
        bytes[4]
    );

    SLOG(NIC, "Got MAC "
        << fmt(macaddr.bytes()[0], "0x", 2) << ":" << fmt(macaddr.bytes()[1], "0x", 2) << ":"
        << fmt(macaddr.bytes()[2], "0x", 2) << ":" << fmt(macaddr.bytes()[3], "0x", 2) << ":"
        << fmt(macaddr.bytes()[4], "0x", 2) << ":" << fmt(macaddr.bytes()[5], "0x", 2) << " from EEPROM");

    return macaddr;
}

bool E1000::linkStateChanged() {
    if(_linkStateChanged) {
        _linkStateChanged = false;
        return true;
    }
    return false;
}

bool E1000::linkIsUp() {
    return readReg(REG_STATUS) & STATUS_LU;
}

EEPROM::EEPROM(E1000& dev)
    : _dev(dev),
      _shift(),
      _doneBit() {
}

bool EEPROM::init() {
    // determine the done bit to test when reading REG_EERD and the shift value
    _dev.writeReg(E1000::REG_EERD, E1000::EERD_START);

    auto t = DTU::get().tsc();
    do {
        uint32_t value = _dev.readReg(E1000::REG_EERD);
        if(value & E1000::EERD_DONE_LARGE) {
            SLOG(NIC, "Detected large EERD");
            _doneBit = E1000::EERD_DONE_LARGE;
            _shift = E1000::EERD_SHIFT_LARGE;
            return true;
        }

        if(value & E1000::EERD_DONE_SMALL) {
            SLOG(NIC, "Detected small EERD");
            _doneBit = E1000::EERD_DONE_SMALL;
            _shift = E1000::EERD_SHIFT_SMALL;
            return true;
        }
    } while ((DTU::get().tsc() - t) < MAX_WAIT_CYCLES);
    return false;
}

bool EEPROM::read(uintptr_t address, uint8_t* data, size_t len) {
    assert((len & ((1 << WORD_LEN_LOG2) - 1)) == 0);
    while(len) {
        if(!readWord(address, data))
            return false;

        // to next word
        data += 1 << WORD_LEN_LOG2;
        address += 1;
        len -= 1 << WORD_LEN_LOG2;
    }
    return true;
}

bool EEPROM::readWord(uintptr_t address, uint8_t* data) {
    uint16_t *data_word = (uint16_t*)data;

    // set address
    _dev.writeReg(E1000::REG_EERD, E1000::EERD_START | (address << _shift));

    // wait for read to complete
    auto t = DTU::get().tsc();
    do {
        uint32_t value = _dev.readReg(E1000::REG_EERD);
        if(~value & _doneBit) {
            continue;
        }

        *data_word = value >> 16;
        return true;
    } while ((DTU::get().tsc() - t) < MAX_WAIT_CYCLES);
    return false;
}

}
