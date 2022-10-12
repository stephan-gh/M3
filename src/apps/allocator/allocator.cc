/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019 Nils Asmussen, Barkhausen Institut
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

#include <m3/Exception.h>
#include <m3/com/MemGate.h>
#include <m3/stream/Standard.h>

using namespace m3;

int main() {
    for(size_t i = 0;; ++i) {
        try {
            MemGate mem = MemGate::create_global(0x1000, MemGate::RW);
            println("Got memory gate :)"_cf);
            mem.write(&i, sizeof(i), 0);
        }
        catch(const Exception &e) {
            eprintln("Allocation {} failed: {}"_cf, i, e.what());
        }
    }
    return 0;
}
