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

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

volatile int wait_for_debugger = 1;

extern "C" void rust_init(int argc, char **argv);
extern "C" void rust_deinit(int status, void *arg);

static bool str_ends_with(const char *str, const char *end) {
    size_t slen = strlen(str);
    size_t elen = strlen(end);
    return slen >= elen && strncmp(str + (slen - elen), end, elen) == 0;
}

extern "C" __attribute__((constructor)) void host_init(int argc, char **argv) {
    char *wait;
    if((wait = getenv("M3_WAIT")) != 0 && argv[0] && str_ends_with(argv[0], wait)) {
        while(wait_for_debugger != 0) {}
    }

    rust_init(argc, argv);
    on_exit(rust_deinit, nullptr);
}
