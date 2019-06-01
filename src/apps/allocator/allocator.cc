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

#include <m3/stream/Standard.h>
#include <m3/com/MemGate.h>

using namespace m3;

int main() {
    for(size_t i = 0; ; ++i) {
        MemGate mem = MemGate::create_global(0x1000, MemGate::RW);
        if(Errors::last == Errors::NONE) {
            mem.write(&i, sizeof(i), 0);
            cout << "Allocation " << i << " succeeded\n";
        }
        else
            cout << "Allocation " << i << " failed: " << Errors::to_string(Errors::last) << "\n";
    }
    return 0;
}
