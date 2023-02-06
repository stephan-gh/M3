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

#include <base/Machine.h>
#include <base/TCU.h>
#include <base/TMIF.h>

#include <string.h>

EXTERN_C void gem5_shutdown(uint64_t delay);
EXTERN_C void gem5_writefile(const char *str, uint64_t len, uint64_t offset, uint64_t file);
EXTERN_C void gem5_resetstats(uint64_t delay, uint64_t period);
EXTERN_C void gem5_dumpstats(uint64_t delay, uint64_t period);

namespace m3 {

void Machine::shutdown() {
    if(bootenv()->platform == Platform::GEM5)
        gem5_shutdown(0);
    else {
        while(1)
            ;
    }
    UNREACHED;
}

ssize_t Machine::write(const char *str, size_t len) {
    size_t amount = TCU::get().print(str, len);
    if(bootenv()->platform == Platform::GEM5) {
        static const char *fileAddr = "stdout";
        // touch the string first to cause a page fault, if required. gem5 assumes that it's mapped
        ((volatile const char *)fileAddr)[0];
        ((volatile const char *)fileAddr)[6];
        gem5_writefile(str, amount, 0, reinterpret_cast<uint64_t>(fileAddr));
    }
    return static_cast<ssize_t>(amount);
}

void Machine::reset_stats() {
    if(bootenv()->platform == Platform::GEM5)
        gem5_resetstats(0, 0);
}

void Machine::dump_stats() {
    if(bootenv()->platform == Platform::GEM5)
        gem5_dumpstats(0, 0);
}

}
