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

#include <base/stream/Serial.h>
#include <base/PEXIF.h>
#include <base/KIF.h>

#include "xaxiethernet.h"
#include "xparameters.h"
#include "xllfifo.h"
#include "sleep.h"

#define PHY_R0_CTRL_REG     0
#define PHY_R3_PHY_IDENT_REG    3

#define PHY_R0_RESET         0x8000
#define PHY_R0_LOOPBACK      0x4000
#define PHY_R0_ANEG_ENABLE   0x1000
#define PHY_R0_DFT_SPD_MASK  0x2040
#define PHY_R0_DFT_SPD_10    0x0000
#define PHY_R0_DFT_SPD_100   0x2000
#define PHY_R0_DFT_SPD_1000  0x0040
#define PHY_R0_ISOLATE       0x0400

static XAxiEthernet AxiEthernetInstance;
static XLlFifo FifoInstance;
static u8 LocalMacAddr[6] = {0x00, 0x0A, 0x35, 0x03, 0x02, 0x03};

static int PhySetup(XAxiEthernet *AxiEthernetInstancePtr) {
    u16 PhyReg0;
    u32 PhyAddr;
    u16 status;

    PhyAddr = XPAR_AXIETHERNET_0_PHYADDR;

    /* Clear the PHY of any existing bits by zeroing this out */
    PhyReg0 = 0;
    XAxiEthernet_PhyRead(AxiEthernetInstancePtr, PhyAddr,
                 PHY_R0_CTRL_REG, &PhyReg0);

    PhyReg0 &= (~PHY_R0_ANEG_ENABLE);
    PhyReg0 &= (~PHY_R0_ISOLATE);
    PhyReg0 |= PHY_R0_DFT_SPD_1000;

    sleep(1);
    XAxiEthernet_PhyWrite(AxiEthernetInstancePtr, PhyAddr,
                PHY_R0_CTRL_REG, PhyReg0);

    XAxiEthernet_PhyRead(AxiEthernetInstancePtr, PhyAddr, 1, &status);

    return XST_SUCCESS;
}

EXTERN_C int axieth_reset() {
    int Status;
    u8 MacSave[6];
    u32 Options;

    xdbg_printf(XDBG_DEBUG_GENERAL, "axieth_reset()\n");

    /*
     * Stop device
     */
    XAxiEthernet_Stop(&AxiEthernetInstance);

    /*
     * Save the device state
     */
    XAxiEthernet_GetMacAddress(&AxiEthernetInstance, MacSave);
    Options = XAxiEthernet_GetOptions(&AxiEthernetInstance);

    /*
     * Stop and reset both the fifo and the AxiEthernet the devices
     */
    XLlFifo_Reset(&FifoInstance);
    XAxiEthernet_Reset(&AxiEthernetInstance);

    /*
     * Restore the state
     */
    Status = XAxiEthernet_SetMacAddress(&AxiEthernetInstance, MacSave);
    Status |= XAxiEthernet_SetOptions(&AxiEthernetInstance, Options);
    Status |= XAxiEthernet_ClearOptions(&AxiEthernetInstance, ~Options);
    if (Status != XST_SUCCESS) {
        xdbg_printf(XDBG_DEBUG_ERROR, "Error restoring state after reset");
        return XST_FAILURE;
    }

    return XST_SUCCESS;
}

EXTERN_C int axieth_init() {
    XAxiEthernet *AxiEthernetInstancePtr = &AxiEthernetInstance;
    XLlFifo *FifoInstancePtr = &FifoInstance;
    u16 AxiEthernetDeviceId = XPAR_AXIETHERNET_0_DEVICE_ID;

    XAxiEthernet_Config *MacCfgPtr;
    int Status;

    m3::Serial::init("net", m3::env()->pe_id);

    xdbg_printf(XDBG_DEBUG_GENERAL, "axieth_init()\n");

    /*
     *  Get the configuration of AxiEthernet hardware.
     */
    MacCfgPtr = XAxiEthernet_LookupConfig(AxiEthernetDeviceId);

    /*
     * Check whether AXIFIFO is present or not
     */
    if(MacCfgPtr->AxiDevType != XPAR_AXI_FIFO) {
        xdbg_printf(XDBG_DEBUG_ERROR, "Device HW not configured for FIFO mode\n");
        return 1;
    }

    // map AXI ethernet MMIO region
    size_t pages = ((XPAR_AXIETHERNET_0_HIGHADDR + 1) - MacCfgPtr->BaseAddress) / PAGE_SIZE;
    m3::PEXIF::map(MacCfgPtr->BaseAddress, MacCfgPtr->BaseAddress, pages, m3::KIF::Perm::RW);

    // map AXI FIFO MMIO region
    m3::PEXIF::map(MacCfgPtr->AxiDevBaseAddress, MacCfgPtr->AxiDevBaseAddress, 64, m3::KIF::Perm::RW);

    XLlFifo_Initialize(FifoInstancePtr, MacCfgPtr->AxiDevBaseAddress);

    /*
     * Initialize AxiEthernet hardware.
     */
    Status = XAxiEthernet_CfgInitialize(AxiEthernetInstancePtr, MacCfgPtr,
                    MacCfgPtr->BaseAddress);
    if (Status != XST_SUCCESS) {
        xdbg_printf(XDBG_DEBUG_ERROR, "Error in initialize\n");
        return 1;
    }

    xdbg_printf(XDBG_DEBUG_GENERAL, "Cfg init success\n");

    /*
     * Set the MAC address
     */
    Status = XAxiEthernet_SetMacAddress(AxiEthernetInstancePtr,
                    LocalMacAddr);
    if (Status != XST_SUCCESS) {
          xdbg_printf(XDBG_DEBUG_ERROR, "Error setting MAC address");
          return 1;
    }

    PhySetup(AxiEthernetInstancePtr);

    /*
     * Setting the operating speed of the MAC needs a delay.  There
     * doesn't seem to be register to poll, so please consider this
     * during your application design.
     */
    sleep(2);

    /*
     * Make sure Tx and Rx are enabled
     */
    Status = XAxiEthernet_SetOptions(AxiEthernetInstancePtr,
                         XAE_RECEIVER_ENABLE_OPTION |
                     XAE_TRANSMITTER_ENABLE_OPTION);
    if (Status != XST_SUCCESS) {
        xdbg_printf(XDBG_DEBUG_ERROR, "Error setting options");
        return 1;
    }

    /* Clear any pending FIFO interrupts from any previous
     * examples (e.g., polled)
     */
    XLlFifo_IntClear(FifoInstancePtr, XLLF_INT_ALL_MASK);

    /*
     * Start the Axi Ethernet and enable its ERROR interrupts
     */
    XAxiEthernet_Start(AxiEthernetInstancePtr);

    /**
     * Enable interrupts
     */
    uint mask = XAE_INT_RXCMPIT_MASK | XAE_INT_RXRJECT_MASK | XAE_INT_RXFIFOOVR_MASK;
    // enable the interrupts only that axieth, because the FIFO does not seem to generate any
    // interrupts. TODO on Linux it does!?
    XAxiEthernet_IntEnable(AxiEthernetInstancePtr, mask);
    m3::PEXIF::reg_irq(XPAR_AXIETHERNET_0_INTR);

    return 0;
}

EXTERN_C int axieth_send(void *packet, size_t len) {
    XLlFifo *FifoInstancePtr = &FifoInstance;

    xdbg_printf(XDBG_DEBUG_GENERAL,
        "axieth_send(packet= " << m3::fmt(packet, "p") << ", len=" << m3::fmt(len, "#x") << ")\n");

    // check for enough room in FIFO
    u32 FifoFreeBytes = XLlFifo_TxVacancy(FifoInstancePtr) * 4;
    if (FifoFreeBytes < len) {
        xdbg_printf(XDBG_DEBUG_ERROR,
            "FIFO has not enough space: need=" << len << ", have=" << FifoFreeBytes << "\n");
        return 1;
    }

    // Write the frame data to FIFO
    XLlFifo_Write(FifoInstancePtr, packet, len);

    // Initiate transmit
    XLlFifo_TxSetLen(FifoInstancePtr, len);

    return 0;
}

EXTERN_C size_t axieth_recv(void *buffer, size_t len) {
    XAxiEthernet *AxiEthernetInstancePtr = &AxiEthernetInstance;
    XLlFifo *FifoInstancePtr = &FifoInstance;

    xdbg_printf(XDBG_DEBUG_GENERAL,
        "axieth_recv(buffer= " << m3::fmt(buffer, "p") << ", len=" << m3::fmt(len, "#x") << ")\n");

    u32 Pending = XAxiEthernet_IntPending(AxiEthernetInstancePtr);

    while (Pending) {
        if (Pending & XAE_INT_RXCMPIT_MASK) {
            XAxiEthernet_IntClear(AxiEthernetInstancePtr, XAE_INT_RXCMPIT_MASK);
        }
        else if (Pending & XAE_INT_RECV_ERROR_MASK) {
            xdbg_printf(XDBG_DEBUG_ERROR, "Receive error\n");
            XAxiEthernet_IntClear(AxiEthernetInstancePtr, XAE_INT_RECV_ERROR_MASK);
        }
        else {
            xdbg_printf(XDBG_DEBUG_ERROR,
                "Unexpected interrupt: " << m3::fmt(Pending, "#x") << "\n");
            XAxiEthernet_IntClear(AxiEthernetInstancePtr, Pending);
        }
        Pending = XAxiEthernet_IntPending(AxiEthernetInstancePtr);
    }

    if(!XLlFifo_iRxOccupancy(FifoInstancePtr))
        return 0;

    u32 RecvFrameLength = XLlFifo_RxGetLen(FifoInstancePtr);
    if(len < RecvFrameLength) {
        xdbg_printf(XDBG_DEBUG_ERROR, "Dropping packet; not enough space\n");
        return 0;
    }

    // read the frame from the FIFO
    XLlFifo_Read(FifoInstancePtr, buffer, RecvFrameLength);

    xdbg_printf(XDBG_DEBUG_GENERAL, "Received packet with " << RecvFrameLength << " bytes\n");

    return RecvFrameLength;
}
