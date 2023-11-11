/*
 * Copyright (C) 2015-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2022 Nils Asmussen, Barkhausen Institut
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

#include <base/Quota.h>
#include <base/util/Reference.h>

#include <m3/cap/ObjCap.h>

namespace m3 {

class KMem : public ObjCap, public RefCounted {
    friend class Activity;

    explicit KMem(capsel_t sel, uint flags) noexcept : ObjCap(KMEM, sel, flags) {
    }

    void set_flags(uint fl) noexcept {
        flags(fl);
    }

public:
    explicit KMem(capsel_t sel) noexcept : KMem(sel, KEEP_CAP) {
    }

    Quota<size_t> quota() const;

    Reference<KMem> derive(const KMem &base, size_t quota);
};

}
