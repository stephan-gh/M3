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

#pragma once

#include <base/Common.h>
#include <m3/net/Net.h>
#include <pci/Device.h>

#define DEBUG_E1000  0
#if DEBUG_E1000
#   include <base/stream/Serial.h>
#   define DBG_E1000(expr) cout << expr
#else
#   define DBG_E1000(...)
#endif

namespace net {

class E1000;

class EEPROM {
    static const size_t WORD_LEN_LOG2    = 1;
    // TODO: Use a sensible value, the current one is chosen arbitrarily
    static const cycles_t MAX_WAIT_CYCLES = 100000;

public:
    EEPROM(E1000 & dev);
    bool init();
    bool read(uintptr_t address, uint8_t *data, size_t len);

private:
    bool readWord(uintptr_t address, uint8_t *data);

    E1000 & _dev;
    int _shift;
    uint32_t _doneBit;
};

class E1000 {
    friend class EEPROM;

    enum {
        REG_CTRL            = 0x0,           /* device control register */
        REG_STATUS          = 0x8,           /* device status register */
        REG_EECD            = 0x10,          /* EEPROM control/data register */
        REG_EERD            = 0x14,          /* EEPROM read register */
        REG_VET             = 0x38,          /* VLAN ether type */

        REG_ICR             = 0xc0,          /* interrupt cause read register */
        REG_IMS             = 0xd0,          /* interrupt mask set/read register */
        REG_IMC             = 0xd8,          /* interrupt mask clear register */

        REG_RCTL            = 0x100,         /* receive control register */
        REG_TCTL            = 0x400,         /* transmit control register */

        REG_PBA             = 0x1000,        /* packet buffer allocation */
        REG_PBS             = 0x1008,        /* packet buffer size */

        REG_RDBAL           = 0x2800,        /* register descriptor base address low */
        REG_RDBAH           = 0x2804,        /* register descriptor base address high */
        REG_RDLEN           = 0x2808,        /* register descriptor length */
        REG_RDH             = 0x2810,        /* register descriptor head */
        REG_RDT             = 0x2818,        /* register descriptor tail */

        REG_RDTR            = 0x2820,        /* receive delay timer register */
        REG_RDCTL           = 0x2828,        /* transmit descriptor control */
        REG_RADV            = 0x282c,        /* receive absolute interrupt delay timer */

        REG_TDBAL           = 0x3800,        /* transmit descriptor base address low */
        REG_TDBAH           = 0x3804,        /* transmit descriptor base address high */
        REG_TDLEN           = 0x3808,        /* transmit descriptor length */
        REG_TDH             = 0x3810,        /* transmit descriptor head */
        REG_TDT             = 0x3818,        /* transmit descriptor tail */

        REG_TIDV            = 0x3820,        /* transmit interrupt delay value */
        REG_TDCTL           = 0x3828,        /* transmit descriptor control */
        REG_TADV            = 0x382c,        /* transmit absolute interrupt delay timer */

        REG_RAL             = 0x5400,        /* filtering: receive address low */
        REG_RAH             = 0x5404,        /* filtering: receive address high */

        REG_RXCSUM          = 0x5000,        /* receive checksum control */
    };

    enum {
        STATUS_LU           = 1 << 1,        /* link up */
    };

    enum {
        CTL_LRST            = 1 << 3,        /* link reset */
        CTL_ASDE            = 1 << 5,        /* auto speed detection enable */
        CTL_SLU             = 1 << 6,        /* set link up */
        CTL_FRCSPD          = 1 << 11,       /* force speed */
        CTL_FRCDPLX         = 1 << 12,       /* force duplex */
        CTL_RESET           = 1 << 26,       /* 1 = device reset; self-clearing */
        CTL_PHY_RESET       = 1 << 31,       /* 1 = PHY reset */
    };

    enum {
        XDCTL_ENABLE        = 1 << 25,       /* queue enable */
    };

    enum {
        ICR_LSC             = 1 << 2,        /* Link Status Change */
        ICR_RXDMT0          = 1 << 4,        /* Receive Descriptor Minimum Threshold Reached */
        ICR_RXO             = 1 << 6,        /* Receiver Overrun */
        ICR_RXT0            = 1 << 7,        /* Receiver Timer Interrupt */
    };

    enum {
        RCTL_ENABLE         = 1 << 1,
        RCTL_UPE            = 1 << 3,        /* unicast promiscuous mode */
        RCTL_MPE            = 1 << 4,        /* multicast promiscuous */
        RCTL_BAM            = 1 << 15,       /* broadcasts accept mode */
        RCTL_BSIZE_256      = 0x11 << 16,    /* receive buffer size = 256 bytes (if RCTL_BSEX = 0) */
        RCTL_BSIZE_512      = 0x10 << 16,    /* receive buffer size = 512 bytes (if RCTL_BSEX = 0) */
        RCTL_BSIZE_1K       = 0x01 << 16,    /* receive buffer size = 1024 bytes (if RCTL_BSEX = 0) */
        RCTL_BSIZE_2K       = 0x00 << 16,    /* receive buffer size = 2048 bytes (if RCTL_BSEX = 0) */
        RCTL_BSIZE_MASK     = 0x11 << 16,    /* mask for buffer size */
        RCTL_BSEX_MASK      = 0x01 << 25,    /* mask for size extension */
        RCTL_SECRC          = 1 << 26,       /* strip CRC */
    };

    enum {
        TCTL_ENABLE         = 1 << 1,
        TCTL_PSP            = 1 << 3,        /* pad short packets */
        TCTL_COLL_TSH       = 0x0F << 4,     /* collision threshold; number of transmission attempts */
        TCTL_COLL_DIST      = 0x40 << 12,    /* collision distance; pad packets to X bytes; 64 here */
        TCTL_COLT_MASK      = 0xff << 4,
        TCTL_COLD_MASK      = 0x3ff << 12,
    };

    enum {
        RAH_VALID           = 1 << 31,       /* marks a receive address filter as valid */
    };

    enum {
        RXCSUM_PCSS_MASK   = 0xff,           /* Packet Checksum Start */
        RXCSUM_IPOFLD      = 1 << 8,         /* IP Checksum Off-load Enable */
        RXCSUM_TUOFLD      = 1 << 9,         /* TCP/UDP Checksum Off-load Enable */
        RXCSUM_IPV6OFL     = 1 << 10,        /* IPv6 Checksum Offload Enable */
    };

    enum {
        EEPROM_OFS_MAC      = 0x0,           /* offset of the MAC in EEPROM */
    };
    enum {
        EERD_START          = 1 << 0,        /* start command */
        EERD_DONE_SMALL     = 1 << 4,        /* read done (small EERD) */
        EERD_DONE_LARGE     = 1 << 1,        /* read done (large EERD) */
        EERD_SHIFT_SMALL    = 8,             /* address shift (small) */
        EERD_SHIFT_LARGE    = 2,             /* address shift (large) */
    };

    enum {
        TX_CMD_EOP          = 0x01,          /* end of packet */
        TX_CMD_IFCS         = 0x02,          /* insert FCS/CRC */
    };

    enum {
        RXDS_PIF   = 1 << 7, /* Passed in-exact filter */
        RXDS_IPCS  = 1 << 6, /* IP Checksum Calculated on Packet */
        RXDS_TCPCS = 1 << 5, /* TCP Checksum Calculated on Packet */
        // Only in gem5 i8254xGBE?!
        RXDS_UDPCS = 1 << 4, /* TCP Checksum Calculated on Packet */
        RXDS_VP    = 1 << 3, /* Packet is 802.1Q (matched VET) */
        RXDS_IXSM  = 1 << 2, /* Ignore Checksum Indication */
        RXDS_EOP   = 1 << 1, /* End of Packet */
        RXDS_DD    = 1 << 0, /* receive descriptor status; indicates that the HW has
                              * finished the descriptor */
    };

    enum {
        RXDE_RXE  = 1 << 7, /* RX Data Error */
        RXDE_IPE  = 1 << 6, /* IP Checksum Error */
        RXDE_TCPE = 1 << 5, /* TCP/UDP Checksum Error */
        RXDE_SEQ  = 1 << 2, /* Sequence Error */
        RXDE_SE   = 1 << 1, /* Symbol Error */
        RXDE_CE   = 1 << 0, /* CRC Error or Alignment Error */
    };

    static const cycles_t RESET_SLEEP_TIME              = 20 * 1000;

    static const size_t MAX_RECEIVE_COUNT_PER_INTERRUPT = 5;

    static const size_t RX_BUF_COUNT                    = 256;
    static const size_t TX_BUF_COUNT                    = 256;
    static const size_t RX_BUF_SIZE                     = 2048;
    static const size_t TX_BUF_SIZE                     = 2048;

    struct TxDesc {
        uint64_t buffer;
        uint16_t length;
        uint8_t checksumOffset;
        uint8_t cmd;
        uint8_t status;
        uint8_t checksumStart;
        uint16_t : 16;
    } PACKED ALIGNED(4);

    // TODO: Allocation details of bit fields are implementation-defined...
    struct TxContextDesc {
        uint64_t IPCSS  : 8,
                 IPCSO  : 8,
                 IPCSE  : 16,
                 TUCSS  : 8,
                 TUCSO  : 8,
                 TUCSE  : 16;
        uint64_t PAYLEN : 20,
                 DTYP   : 4, // 0000b
                 TUCMD  : 8,
                 STA    : 4,
                 RSV    : 4,
                 HDRLEN : 8,
                 MSS    : 16;

    } PACKED ALIGNED(4);
    static_assert(sizeof(TxContextDesc) == sizeof(TxDesc));

    struct TxDataDesc {
        uint64_t buffer;
        uint64_t length  : 20,
                 DTYP    : 4, // 0001b
                 DCMD    : 8,
                 STA     : 4,
                 RSV     : 4,
                 POPTS   : 8,
                 Special : 16;
    } PACKED ALIGNED(4);
    static_assert(sizeof(TxDataDesc) == sizeof(TxDesc));

     struct RxDesc {
        uint64_t buffer;
        uint16_t length;
        uint16_t checksum;
        uint8_t status;
        uint8_t error;
        uint16_t : 16;
    } PACKED ALIGNED(4);

    struct Buffers {
        RxDesc rxDescs[RX_BUF_COUNT] ALIGNED(16); // 0
        TxDesc txDescs[TX_BUF_COUNT] ALIGNED(16); // 128
        uint8_t rxBuf[RX_BUF_COUNT * RX_BUF_SIZE]; // 16384 + 256
        uint8_t txBuf[TX_BUF_COUNT * TX_BUF_SIZE];
    };

    enum TxoProto {
        TxoProto_Unsupported = 1 << 1, // Protocol with no checksum offload support (never requires context descriptor update)
        TxoProto_IP =          1 << 2 | TxoProto_Unsupported,
        TxoProto_UDP =         1 << 3 | TxoProto_IP,
        TxoProto_TCP =         1 << 4 | TxoProto_IP,
    };

public:
    typedef bool(&alloc_cb_func)(void *&pkt, void *&buf, size_t &bufSize, size_t size);
    typedef void(&next_buf_cb_func)(void *&pkt, void *&buf, size_t &bufSize);
    typedef void(&recv_cb_func)(void *pkt);

    explicit E1000(pci::ProxiedPciDevice & nic, alloc_cb_func allocCallback, next_buf_cb_func nextBufCallback, recv_cb_func recvCallback);

    ulong mtu() const {
        return TX_BUF_SIZE;
    }

    void stop();
    void reset();
    bool send(const void *packet, size_t size);
    void receive(size_t maxReceiveCount);

    void receiveInterrupt();
    m3::net::MAC readMAC();

    bool linkStateChanged();
    bool linkIsUp();

private:
    void sleep(cycles_t usec);

    void writeReg(uint16_t reg,uint32_t value);
    uint32_t readReg(uint16_t reg);
    void readEEPROM(uintptr_t address, uint8_t *dest, size_t len);

    pci::ProxiedPciDevice & _nic;
    EEPROM _eeprom;
    m3::net::MAC _mac;
    uint32_t _curRxBuf;
    uint32_t _curTxDesc;
    uint32_t _curTxBuf;
    m3::MemGate _bufs;
    alloc_cb_func _allocCallback;
    next_buf_cb_func _nextBufCallback;
    recv_cb_func _recvCallback;
    bool _linkStateChanged;
    TxoProto _txdContextProto;
};

}
