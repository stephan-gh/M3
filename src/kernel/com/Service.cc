/*
 * Copyright (C) 2016-2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <base/Common.h>

#include "com/Service.h"
#include "pes/VPE.h"

namespace kernel {

Service::Service(VPE &vpe, const m3::String &name, const m3::Reference<RGateObject> &rgate)
    : RefCounted(),
      _vpe(vpe),
      _squeue(vpe.desc()),
      _name(name),
      _rgate(rgate) {
}

int Service::pending() const {
    return _squeue.inflight() + _squeue.pending();
}

const m3::DTU::Message *Service::send_receive(label_t ident, const void *msg, size_t size, bool free) {
    if(!_rgate->activated())
        return nullptr;

    event_t event = _squeue.send(_rgate->ep, ident, msg, size, free);

    m3::ThreadManager::get().wait_for(event);

    return reinterpret_cast<const m3::DTU::Message*>(m3::ThreadManager::get().get_current_msg());
}

}
