/*
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

#include <base/Common.h>
#include <base/Panic.h>
#include <base/time/Profile.h>

#include <m3/Test.h>
#include <m3/com/GateStream.h>

#include "../cppbenchs.h"

using namespace m3;

static const int msg_ord = nextlog2<256>::val;

NOINLINE static void pingpong_1u64() {
    auto rgate = RecvGate::create(msg_ord, msg_ord);
    auto sgate = SendGate::create(&rgate, SendGateArgs().credits(1));

    Profile pr;
    WVPERF(__func__, pr.run<CycleInstant>([&sgate, &rgate] {
        send_vmsg(sgate, 0);

        uint64_t res;
        auto msg = receive_msg(rgate);
        msg >> res;
        if(res != 0)
            panic("test failed"_cf);
        reply_vmsg(msg, 0);

        auto reply = receive_msg(*sgate.reply_gate());
        reply >> res;
        if(res != 0)
            panic("test failed"_cf);
    }));
}

NOINLINE static void pingpong_2u64() {
    auto rgate = RecvGate::create(msg_ord, msg_ord);
    auto sgate = SendGate::create(&rgate, SendGateArgs().credits(1));

    Profile pr;
    WVPERF(__func__, pr.run<CycleInstant>([&sgate, &rgate] {
        send_vmsg(sgate, 23, 42);

        uint64_t res1, res2;
        auto msg = receive_msg(rgate);
        msg >> res1 >> res2;
        if(res1 != 23 || res2 != 42)
            panic("test failed"_cf);
        reply_vmsg(msg, 5, 6);

        auto reply = receive_msg(*sgate.reply_gate());
        reply >> res1 >> res2;
        if(res1 != 5 || res2 != 6)
            panic("test failed"_cf);
    }));
}

NOINLINE static void pingpong_4u64() {
    auto rgate = RecvGate::create(msg_ord, msg_ord);
    auto sgate = SendGate::create(&rgate, SendGateArgs().credits(1));

    Profile pr;
    WVPERF(__func__, pr.run<CycleInstant>([&sgate, &rgate] {
        send_vmsg(sgate, 23, 42, 10, 12);

        uint64_t res1, res2, res3, res4;
        auto msg = receive_msg(rgate);
        msg >> res1 >> res2 >> res3 >> res4;
        if(res1 != 23 || res2 != 42 || res3 != 10 || res4 != 12)
            panic("test failed"_cf);
        reply_vmsg(msg, 5, 6, 7, 8);

        auto reply = receive_msg(*sgate.reply_gate());
        reply >> res1 >> res2 >> res3 >> res4;
        if(res1 != 5 || res2 != 6 || res3 != 7 || res4 != 8)
            panic("test failed"_cf);
    }));
}

NOINLINE static void pingpong_str() {
    auto rgate = RecvGate::create(msg_ord, msg_ord);
    auto sgate = SendGate::create(&rgate, SendGateArgs().credits(1));

    Profile pr(100, 100);
    WVPERF(__func__, pr.run<CycleInstant>([&sgate, &rgate] {
        send_vmsg(sgate, "test");

        std::string res;
        auto msg = receive_msg(rgate);
        msg >> res;
        if(res.length() != 4)
            panic("test failed"_cf);
        reply_vmsg(msg, "foobar");

        auto reply = receive_msg(*sgate.reply_gate());
        reply >> res;
        if(res.length() != 6)
            panic("test failed"_cf);
    }));
}

NOINLINE static void pingpong_strref() {
    auto rgate = RecvGate::create(msg_ord, msg_ord);
    auto sgate = SendGate::create(&rgate, SendGateArgs().credits(1));

    Profile pr;
    WVPERF(__func__, pr.run<CycleInstant>([&sgate, &rgate] {
        send_vmsg(sgate, "test");

        std::string_view res;
        auto msg = receive_msg(rgate);
        msg >> res;
        if(res.length() != 4)
            panic("test failed"_cf);
        reply_vmsg(msg, "foobar");

        auto reply = receive_msg(*sgate.reply_gate());
        reply >> res;
        if(res.length() != 6)
            panic("test failed"_cf);
    }));
}

void bstream() {
    RUN_BENCH(pingpong_1u64);
    RUN_BENCH(pingpong_2u64);
    RUN_BENCH(pingpong_4u64);
    RUN_BENCH(pingpong_str);
    RUN_BENCH(pingpong_strref);
}
