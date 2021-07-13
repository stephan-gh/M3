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

static constexpr size_t MSG_SIZE = 64;
static constexpr epid_t DSTEP = TCU::FIRST_USER_EP;
static constexpr epid_t SEP = TCU::FIRST_USER_EP;
static constexpr epid_t REP = TCU::FIRST_USER_EP + 1;

static uint8_t rbuf[64];

int main() {
    kernel::TCU::config_send(SEP, 0x1234, pe_id(PE::PE0), DSTEP, nextlog2<MSG_SIZE>::val, 1);
    size_t size = nextlog2<sizeof(rbuf)>::val;
    uintptr_t rbuf_addr = reinterpret_cast<uintptr_t>(rbuf);
    kernel::TCU::config_recv(REP, rbuf_addr, size, size, TCU::NO_REPLIES);

    MsgBuf msg;
    msg.cast<uint64_t>() = 0;

    Serial::get() << "Hello World from sender!\n";

    // initial send; wait until receiver is ready
    Errors::Code res;
    while((res = kernel::TCU::send(SEP, msg, 0x2222, REP)) != Errors::NONE) {
        Serial::get() << "send failed: " << res << "\n";
        // get credits back
        kernel::TCU::config_send(SEP, 0x1234, pe_id(PE::PE0), DSTEP, nextlog2<MSG_SIZE>::val, 1);
    }

    for(int count = 0; count < 100000; ++count) {
        // wait for reply
        const TCU::Message *rmsg;
        while((rmsg = kernel::TCU::fetch_msg(REP, rbuf_addr)) == nullptr)
            ;
        ASSERT_EQ(rmsg->label, 0x2222);

        // ack reply
        ASSERT_EQ(kernel::TCU::ack_msg(REP, rbuf_addr, rmsg), Errors::NONE);

        // send message
        ASSERT_EQ(kernel::TCU::send(SEP, msg, 0x2222, REP), Errors::NONE);
        msg.cast<uint64_t>() += 1;
    }
    return 0;
}
