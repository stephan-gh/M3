/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2020 Nils Asmussen, Barkhausen Institut
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

#include <m3/Syscalls.h>
#include <m3/com/RecvGate.h>
#include <m3/stream/Standard.h>

using namespace m3;

int main() {
    capsel_t sel = 1000;

    RecvGate rgate = RecvGate::create(nextlog2<512>::val, nextlog2<64>::val);
    while(1) {
        try {
            m3::Syscalls::create_sgate(sel++, rgate.sel(), 0, SendGate::UNLIMITED);
        }
        catch(const Exception &e) {
            eprintln("Unable to create sgate: {}"_cf, e.what());
        }
    }
    return 0;
}
