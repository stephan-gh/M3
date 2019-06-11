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

#include <base/Common.h>
#include <base/util/Profile.h>
#include <base/Panic.h>

#include <m3/com/GateStream.h>
#include <m3/stream/Standard.h>

#include "../cppbenchs.h"

using namespace m3;

static const size_t msg_size = 128;
static const int msg_ord     = nextlog2<msg_size>::val;

NOINLINE static void pingpong_1u64() {
    auto rgate = RecvGate::create(msg_ord, msg_ord);
    rgate.activate();
    auto sgate = SendGate::create(&rgate, 0, msg_size);

    Profile pr;
    cout << pr.run_with_id([&sgate, &rgate] {
        send_vmsg(sgate, 0);

        uint64_t res;
        auto msg = receive_msg(rgate);
        msg >> res;
        if(res != 0)
            PANIC("test failed");
        reply_vmsg(msg, 0);

        auto reply = receive_msg(*sgate.reply_gate());
        reply >> res;
        if(res != 0)
            PANIC("test failed");
    }, 0x90) << "\n";
}

NOINLINE static void pingpong_2u64() {
    auto rgate = RecvGate::create(msg_ord, msg_ord);
    rgate.activate();
    auto sgate = SendGate::create(&rgate, 0, msg_size);

    Profile pr;
    cout << pr.run_with_id([&sgate, &rgate] {
        send_vmsg(sgate, 23, 42);

        uint64_t res1, res2;
        auto msg = receive_msg(rgate);
        msg >> res1 >> res2;
        if(res1 != 23 || res2 != 42)
            PANIC("test failed");
        reply_vmsg(msg, 5, 6);

        auto reply = receive_msg(*sgate.reply_gate());
        reply >> res1 >> res2;
        if(res1 != 5 || res2 != 6)
            PANIC("test failed");
    }, 0x91) << "\n";
}

NOINLINE static void pingpong_4u64() {
    auto rgate = RecvGate::create(msg_ord, msg_ord);
    rgate.activate();
    auto sgate = SendGate::create(&rgate, 0, msg_size);

    Profile pr;
    cout << pr.run_with_id([&sgate, &rgate] {
        send_vmsg(sgate, 23, 42, 10, 12);

        uint64_t res1, res2, res3, res4;
        auto msg = receive_msg(rgate);
        msg >> res1 >> res2 >> res3 >> res4;
        if(res1 != 23 || res2 != 42 || res3 != 10 || res4 != 12)
            PANIC("test failed");
        reply_vmsg(msg, 5, 6, 7, 8);

        auto reply = receive_msg(*sgate.reply_gate());
        reply >> res1 >> res2 >> res3 >> res4;
        if(res1 != 5 || res2 != 6 || res3 != 7 || res4 != 8)
            PANIC("test failed");
    }, 0x92) << "\n";
}

NOINLINE static void pingpong_str() {
    auto rgate = RecvGate::create(msg_ord, msg_ord);
    rgate.activate();
    auto sgate = SendGate::create(&rgate, 0, msg_size);

    Profile pr;
    cout << pr.run_with_id([&sgate, &rgate] {
        send_vmsg(sgate, "test");

        String res;
        auto msg = receive_msg(rgate);
        msg >> res;
        if(res.length() != 4)
            PANIC("test failed");
        reply_vmsg(msg, "foobar");

        auto reply = receive_msg(*sgate.reply_gate());
        reply >> res;
        if(res.length() != 6)
            PANIC("test failed");
    }, 0x93) << "\n";
}

void bstream() {
    RUN_BENCH(pingpong_1u64);
    RUN_BENCH(pingpong_2u64);
    RUN_BENCH(pingpong_4u64);
    RUN_BENCH(pingpong_str);
}
