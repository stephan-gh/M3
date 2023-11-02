/*
 * Copyright (C) 2020-2022 Nils Asmussen, Barkhausen Institut
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

#include <base/arch/m3/Init.h>

#include "common.h"

using namespace m3;

extern "C" int main(int argc, char **argv);

extern "C" void env_run() {
    const auto [argc, argv] = init(false);
    Serial::init("standalone", TileId::from_raw(bootenv()->tile_id));

    int res = main(argc, argv);

    deinit();
    ::exit(res);
}

namespace m3 {
NORETURN void __exit(int) {
    Machine::shutdown();
}
}
