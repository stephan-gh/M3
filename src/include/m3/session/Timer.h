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

#include <m3/com/Gate.h>
#include <m3/com/RecvGate.h>
#include <m3/session/ClientSession.h>
#include <m3/tiles/Activity.h>

namespace m3 {

class Timer : public ClientSession {
public:
    explicit Timer(const std::string_view &service, uint buford = nextlog2<256>::val,
                   uint msgord = nextlog2<64>::val)
        : ClientSession(service),
          _rgate(RecvGate::create(buford, msgord)),
          _scap(SendCap::create(&_rgate)) {
        delegate_obj(_scap.sel());
    }

    RecvGate &rgate() noexcept {
        return _rgate;
    }

private:
    RecvGate _rgate;
    SendCap _scap;
};

}
