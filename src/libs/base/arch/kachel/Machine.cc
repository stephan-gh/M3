/*
 * Copyright (C) 2016-2017, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <base/Machine.h>
#include <base/TCU.h>
#include <base/PEXIF.h>
#include <string.h>

EXTERN_C void gem5_shutdown(uint64_t delay);
EXTERN_C void gem5_writefile(const char *str, uint64_t len, uint64_t offset, uint64_t file);
EXTERN_C ssize_t gem5_readfile(char *dst, uint64_t max, uint64_t offset);
EXTERN_C void gem5_resetstats(uint64_t delay, uint64_t period);
EXTERN_C void gem5_dumpstats(uint64_t delay, uint64_t period);

namespace m3 {

void Machine::shutdown() {
    if(env()->platform == Platform::GEM5)
        gem5_shutdown(0);
    else {
        while(1)
            ;
    }
    UNREACHED;
}

ssize_t Machine::write(const char *str, size_t len) {
    if(env()->platform == Platform::GEM5) {
        TCU::get().print(str, len);
        static const char *fileAddr = "stdout";
        gem5_writefile(str, len, 0, reinterpret_cast<uint64_t>(fileAddr));
    }
    else {
        if(env()->pe_id == 0) {
            TCU::get().write(127, str, len, 0);
        }
        else {
            PEXIF::print(str, len);
        }
    }
    return static_cast<ssize_t>(len);
}

ssize_t Machine::read(char *dst, size_t max) {
    if(env()->platform == Platform::GEM5)
        return gem5_readfile(dst, max, 0);
    // TODO
    return 0;
}

void Machine::reset_stats() {
    if(env()->platform == Platform::GEM5)
        gem5_resetstats(0, 0);
}

void Machine::dump_stats() {
    if(env()->platform == Platform::GEM5)
        gem5_dumpstats(0, 0);
}

}
