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

#include <base/TileDesc.h>

#include <m3/WorkLoop.h>
#include <m3/com/MemGate.h>
#include <m3/com/RecvGate.h>
#include <m3/com/SendGate.h>
#include <m3/tiles/ChildActivity.h>

namespace pci {

class ProxiedPciDevice {
public:
    static const uint EP_INT = 16;
    static const uint EP_DMA = 17;

    // Hardcoded for now
    static const size_t REG_SIZE = 128 * 1024;
    static const size_t REG_ADDR = 0x4000;
    static const size_t PCI_CFG_ADDR = 0xF00'0000;

    explicit ProxiedPciDevice(const char *name, m3::TileISA isa);

    template<typename T>
    T readReg(size_t offset) {
        T val;
        _mem.read(&val, sizeof(T), REG_ADDR + offset);
        return val;
    }
    template<typename T>
    void writeReg(size_t offset, T val) {
        _mem.write(&val, sizeof(T), REG_ADDR + offset);
    }

    template<typename T>
    T readConfig(uintptr_t offset) {
        T val;
        _mem.read(&val, sizeof(val), REG_ADDR + PCI_CFG_ADDR + offset);
        return val;
    }
    template<typename T>
    void writeConfig(uintptr_t offset, T val) {
        _mem.write(&val, sizeof(val), REG_ADDR + PCI_CFG_ADDR + offset);
    }

    void setDmaEp(m3::MemCap &memcap);

    void listenForIRQs(m3::WorkLoop *wl, std::function<void()> callback);
    void stopListing();

    void waitForIRQ() {
        const m3::TCU::Message *msg = _intgate.receive(nullptr);
        _intgate.ack_msg(msg);
    }

    /**
     * @return the activity for the proxied pci device
     */
    m3::Activity &act() {
        return _act;
    }

private:
    static void receiveInterrupt(ProxiedPciDevice *nic, m3::GateIStream &is);

    m3::Reference<m3::Tile> _tile;
    m3::ChildActivity _act;
    m3::MemGate _mem;
    m3::EP _sep;
    m3::EP _mep;
    m3::RecvGate _intgate; // receives interrupts from the proxied pci device
    m3::SendCap _sintgate; // used by the proxied pci device to signal interrupts to its driver
    std::function<void()> _callback;
};

}
