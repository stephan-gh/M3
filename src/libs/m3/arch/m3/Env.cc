/*
 * Copyright (C) 2016-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <base/arch/m3/Init.h>
#include <base/Common.h>
#include <base/TMIF.h>
#include <base/stream/Serial.h>

#include <m3/Env.h>
#include <m3/Exception.h>

EXTERN_C int main(int argc, char **argv);

namespace m3 {

extern "C" void env_run() {
    const auto [argc, argv] = init();

    Env *e = env();

    std::set_terminate(Exception::terminate_handler);
    Serial::init(reinterpret_cast<char *>(argv[0]), TileId::from_raw(e->tile_id));

    int res;
    if(e->lambda) {
        auto func = reinterpret_cast<int (*)()>(e->lambda);
        res = (*func)();
    }
    else
        res = ::main(argc, argv);

    deinit();
    ::exit(res);
}

NORETURN void __exit(int code) {
    TMIF::exit(code == 0 ? Errors::SUCCESS : Errors::UNSPECIFIED);
    UNREACHED;
}

}
