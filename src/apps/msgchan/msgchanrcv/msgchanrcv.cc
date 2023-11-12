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

#include <m3/com/GateStream.h>
#include <m3/com/RecvGate.h>
#include <m3/stream/Standard.h>

using namespace m3;

int main() {
    RecvGate rgate = RecvGate::create_named("chan");

    SendGate s1 = SendGate::create_named("reply1");
    SendGate s2 = SendGate::create_named("reply2");

    uint64_t val;
    while(1) {
        auto is = receive_msg(rgate);
        is >> val;
        reply_vmsg(is, 0);

        if(is.label<uint64_t>() == 1)
            send_receive_vmsg(s1, val + 1);
        else
            send_receive_vmsg(s2, val + 1);
    }
    return 0;
}
