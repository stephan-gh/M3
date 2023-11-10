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

#include <m3/com/GateStream.h>
#include <m3/session/ClientSession.h>
#include <m3/stream/Standard.h>
#include <m3/tiles/Activity.h>

using namespace m3;

static void received_data(GateIStream &is) {
    auto data = reinterpret_cast<const uint64_t *>(is.buffer());
    println("{}: received {:x}"_cf, env()->tile_id, *data);
}

int main() {
    ClientSession qtest("queuetest");

    WorkLoop wl;

    RecvGate rgate = RecvGate::create(nextlog2<4096>::val, nextlog2<512>::val);
    SendCap scap = SendCap::create(&rgate);
    qtest.delegate_obj(scap.sel());
    rgate.start(&wl, received_data);

    wl.run();
    return 0;
}
