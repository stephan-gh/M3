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
#include <base/stream/Serial.h>

#include "../assert.h"
#include "../pes.h"
#include "../tcuif.h"

using namespace m3;

static constexpr size_t MSG_SIZE = 256;

static uint8_t rbuf[64];

int main() {
    kernel::TCU::config_send(0, 0x1234, pe_id(PE::PE1), 0, nextlog2<MSG_SIZE>::val, 1);
    size_t size = nextlog2<sizeof(rbuf)>::val;
    uintptr_t rbuf_addr = reinterpret_cast<uintptr_t>(rbuf);
    kernel::TCU::config_recv(1, rbuf_addr, size, size, TCU::NO_REPLIES);

    for(volatile int i = 0; i < 1000; ++i)
        ;

    Serial::get() << "Hello World from sender!\n";

    uint64_t msg = 0xDEADBEEF;
    while(1) {
        // send message
        ASSERT_EQ(kernel::TCU::send(0, &msg, sizeof(msg), 0x2222, 1), Errors::NONE);
        msg++;

        // wait for reply
        const TCU::Message *rmsg;
        while((rmsg = kernel::TCU::fetch_msg(1, rbuf_addr)) == nullptr)
            ;
        const uint64_t *data = reinterpret_cast<const uint64_t*>(rmsg->data);
        Serial::get() << "Got reply: "
            << "label=" << fmt(rmsg->label, "#x")
            << ", payload=" << fmt(data[0], "#x") << "\n";

        // ack reply
        ASSERT_EQ(kernel::TCU::ack_msg(1, rbuf_addr, rmsg), Errors::NONE);
    }
    return 0;
}
