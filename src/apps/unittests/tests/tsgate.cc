/*
 * Copyright (C) 2020 Nils Asmussen, Barkhausen Institut
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
#include <base/util/Math.h>

#include <m3/Test.h>
#include <m3/com/GateStream.h>

#include "../unittests.h"

using namespace m3;

static const int msg_ord = nextlog2<256>::val;

static void send_errors() {
    auto rgate = RecvGate::create(msg_ord, msg_ord);
    auto sgate = SendGate::create(&rgate, SendGateArgs());

    {
        send_vmsg(sgate, 1, 2);

        int a, b;
        auto msg = receive_msg(rgate);
        msg >> a >> b;

        try {
            msg >> a;
            WVASSERT(false);
        }
        catch(const Exception &e) {
            WVASSERTEQ(e.code(), Errors::INV_ARGS);
        }
    }

    {
        send_vmsg(sgate, 1);

        auto msg = receive_msg(rgate);

        try {
            std::string s;
            msg >> s;
            WVASSERT(false);
        }
        catch(const Exception &e) {
            WVASSERTEQ(e.code(), Errors::INV_ARGS);
        }
    }

    {
        send_vmsg(sgate, 0, "123");

        auto msg = receive_msg(rgate);

        try {
            std::string s;
            msg >> s;
            WVASSERT(false);
        }
        catch(const Exception &e) {
            WVASSERTEQ(e.code(), Errors::INV_ARGS);
        }
    }
}

void tsgate() {
    RUN_TEST(send_errors);
}
