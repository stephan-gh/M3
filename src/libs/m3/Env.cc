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

#include <base/Backtrace.h>
#include <base/Common.h>
#include <base/Env.h>
#include <base/TMIF.h>
#include <base/stream/Serial.h>

#include <m3/Syscalls.h>
#include <m3/com/RecvGate.h>
#include <m3/stream/Standard.h>
#include <m3/tiles/Activity.h>

namespace m3 {

class EnvUserBackend : public EnvBackend {
public:
    explicit EnvUserBackend() {
    }

    virtual void init() override {
        uint64_t *argv = reinterpret_cast<uint64_t *>(env()->argv);
        Serial::init(reinterpret_cast<char *>(argv[0]), TileId::from_raw(env()->tile_id));
    }

    NORETURN void exit(Errors::Code code) override {
        TMIF::exit(code);
        UNREACHED;
    }
};

void Env::init() {
    std::set_terminate(Exception::terminate_handler);
    env()->set_backend(new EnvUserBackend());
    env()->backend()->init();
    env()->call_constr();
}

}
