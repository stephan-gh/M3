/*
 * Copyright (C) 2015-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2021 Nils Asmussen, Barkhausen Institut
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

#include <base/Env.h>
#include <base/Machine.h>
#include <base/stream/Serial.h>

#include <sys/file.h>
#include <unistd.h>

namespace m3 {

void Machine::shutdown() {
    exit(EXIT_FAILURE);
}

ssize_t Machine::write(const char *str, size_t len) {
    return ::write(env()->log_fd(), str, len);
}

void Machine::reset_stats() {
    // not supported
}

void Machine::dump_stats() {
    // not supported
}

}
