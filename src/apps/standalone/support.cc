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

#include <string.h>

#include "DTU.h"

extern "C" int main();
extern "C" void gem5_shutdown(uint64_t delay);
extern "C" void gem5_writefile(const char *str, uint64_t len, uint64_t offset, uint64_t file);

namespace m3 {
class Env {
    __attribute__((used)) void exit(int, bool) {
        gem5_shutdown(0);
    }
};
}

extern "C" int puts(const char *str) {
    static const char *fileAddr = "stdout";
    gem5_writefile(str, strlen(str), 0, reinterpret_cast<uint64_t>(fileAddr));
    return 0;
}

extern "C" void exit(int) {
    gem5_shutdown(0);
}

extern "C" void env_run() {
    exit(main());
}
