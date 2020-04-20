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

#include <base/col/SList.h>
#include <base/util/String.h>
#include <base/Config.h>

#include "Types.h"

namespace kernel {

class KMemCapability;
class PECapability;
class VPE;
class VPECapability;

class VPEManager {
    friend class VPE;
    friend class ContextSwitcher;

    struct Pending : public m3::SListItem {
        explicit Pending(VPE *_vpe) : vpe(_vpe) {
        }

        VPE *vpe;
    };

public:
    static void create() {
        _inst = new VPEManager();
    }
    static VPEManager &get() {
        return *_inst;
    }
    static void destroy() {
        if(_inst) {
            delete _inst;
            _inst = nullptr;
        }
    }

private:
    explicit VPEManager();
    ~VPEManager();

public:
    void start_root();

    VPE *create(m3::String &&name, PECapability *pecap, KMemCapability *kmemcap, epid_t eps_start);

    bool exists(vpeid_t id) const {
        return id < MAX_VPES && _vpes[id];
    }

    VPE &vpe(vpeid_t id) {
        assert(exists(id));
        return *_vpes[id];
    }

#if defined(__host__)
    int pid_by_pe(peid_t pe) const;
    VPE *vpe_by_pid(int pid);
#endif

private:
    vpeid_t get_id() const;

    void add(VPECapability *vpe);
    void remove(VPE *vpe);

    mutable vpeid_t _next_id;
    VPE **_vpes;
    size_t _count;
    static VPEManager *_inst;
};

}
