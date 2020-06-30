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
#include <base/util/Util.h>

#include "../assert.h"
#include "../tcuif.h"

using namespace m3;

static uint8_t rbuf[256];

int main() {
    size_t size = nextlog2<sizeof(rbuf)>::val;
    uintptr_t rbuf_addr = reinterpret_cast<uintptr_t>(rbuf);
    kernel::TCU::config_recv(0, rbuf_addr, size, size, 1);

    for(volatile int i = 0; i < 1000; ++i)
        ;

    Serial::get() << "Hello World from receiver!\n";

    uint64_t reply = 0xCAFEBABE;
    while(1) {
        // wait for message
        const TCU::Message *rmsg;
        while((rmsg = kernel::TCU::fetch_msg(0, rbuf_addr)) == nullptr)
            ;
        const uint64_t *data = reinterpret_cast<const uint64_t*>(rmsg->data);
        Serial::get() << "Got message: "
            << "label=" << fmt(rmsg->label, "#x")
            << ", payload=" << fmt(data[0], "#x") << "\n";
        // const uint64_t *words = reinterpret_cast<const uint64_t*>(rmsg);
        // for(size_t i = 0; i < 8; ++i)
        //     Serial::get() << "word" << i << ": " << fmt(words[i], "#x") << "\n";

        // send reply
        ASSERT_EQ(kernel::TCU::reply(0, &reply, sizeof(reply), rbuf_addr, rmsg), Errors::NONE);
        reply++;
    }
    return 0;
}
