/*
 * Copyright (C) 2023 Nils Asmussen, Barkhausen Institut
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

#include <base/Common.h>
#include <base/Init.h>
#include <base/arch/linux/Init.h>
#include <base/stream/Serial.h>

#include <m3/Env.h>
#include <m3/Exception.h>

namespace m3lx {

struct LinuxEnv {
    LinuxEnv();
};

static INIT_PRIO_LXENV LinuxEnv lxenv;

void lambda_dummy() {
}

LinuxEnv::LinuxEnv() {
    m3::Env *e = m3::env();

    std::set_terminate(m3::Exception::terminate_handler);
    char **argv = reinterpret_cast<char **>(e->argv);
    m3::Serial::init(argv[0], m3::TileId::from_raw(e->tile_id));

    if(e->lambda) {
        auto func = reinterpret_cast<int (*)()>(e->lambda);
        int res = (*func)();
        ::exit(res);
    }
}

}
