/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
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
#include <m3/com/RecvGate.h>
#include <m3/com/SendGate.h>
#include <m3/stream/Standard.h>
#include <m3/tiles/Activity.h>

using namespace m3;

int main(int argc, char **argv) {
    // send a message to ourself, but don't fetch it
    RecvGate rgate = RecvGate::create(nextlog2<512>::val, nextlog2<64>::val);
    SendGate sgate = SendGate::create(&rgate);
    send_vmsg(sgate, 1, 2, 3);

    // now try to trick TileMux to leave us running, because we have unread messages
    for(int i = 0;; ++i) {
        println("Hello {} from {}"_cf, i, argc > 0 ? argv[1] : "??");
        OwnActivity::sleep_for(TimeDuration::from_nanos(10));
    }
    return 0;
}
