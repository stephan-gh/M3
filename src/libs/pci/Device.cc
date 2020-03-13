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

#include <base/Panic.h>

#include <m3/stream/Standard.h>
#include <m3/Syscalls.h>

#include <pci/Device.h>

using namespace m3;

namespace pci {

ProxiedPciDevice::ProxiedPciDevice(const char *name, PEISA isa)
    : _pe(PE::alloc(PEDesc(PEType::COMP_IMEM, isa))),
      _vpe(_pe, name),
      _sep(_vpe.epmng().acquire(EP_INT)),
      _mep(_vpe.epmng().acquire(EP_DMA)),
      _intgate(RecvGate::create(nextlog2<256>::val, nextlog2<32>::val)),
      // TODO: Specify receive gate, grant it to nic tcu, send replies to give credits back
      _sintgate(SendGate::create(&_intgate)) {
    _intgate.activate();

    _vpe.delegate(KIF::CapRngDesc(KIF::CapRngDesc::OBJ, _sintgate.sel(), 1));
    _sintgate.activate_on(*_sep);

    _vpe.start();
}

void ProxiedPciDevice::listenForIRQs(WorkLoop *wl, std::function<void()> callback) {
    _intgate.start(wl, std::bind(receiveInterrupt, this, std::placeholders::_1));
    _callback = callback;
}

void ProxiedPciDevice::stopListing() {
    _intgate.stop();
}

void ProxiedPciDevice::setDmaEp(m3::MemGate &memgate) {
    memgate.activate_on(*_mep);
}

void ProxiedPciDevice::receiveInterrupt(ProxiedPciDevice *nic, m3::GateIStream &) {
    // TODO acknowledge IRQs by sending a reply?
    if(nic->_callback)
        nic->_callback();
    else
        cout << "received interrupt, but no callback is registered.\n";
}

}
