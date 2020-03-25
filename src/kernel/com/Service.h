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

#pragma once

#include <base/Common.h>
#include <base/util/String.h>
#include <base/util/Reference.h>
#include <base/TCU.h>

#include "mem/SlabCache.h"
#include "SendQueue.h"

namespace kernel {

class VPE;
class RGateObject;

class Service : public SlabObject<Service>, public m3::RefCounted {
public:
    explicit Service(VPE &vpe, const m3::String &name, const m3::Reference<RGateObject> &rgate);

    VPE &vpe() const {
        return _vpe;
    }
    const m3::String &name() const {
        return _name;
    }
    const m3::Reference<RGateObject> &rgate() const {
        return _rgate;
    }

    int pending() const;

    const m3::TCU::Message *send_receive(label_t ident, const void *msg, size_t size, bool free);

    void drop_msgs(label_t ident) {
        _squeue.drop_msgs(ident);
    }
    void abort() {
        _squeue.abort();
    }

private:
    VPE &_vpe;
    SendQueue _squeue;
    m3::String _name;
    m3::Reference<RGateObject> _rgate;
};

}
