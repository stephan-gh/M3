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

#include "../common.h"

using namespace m3;

static void test_inv_ep() {
    char rbuffer[32];
    uintptr_t buf = reinterpret_cast<uintptr_t>(&rbuffer);

    MsgBuf msg;
    msg.cast<uint64_t>() = 0xDEADBEEF;

    Serial::get() << "force invalidation\n";
    {
        uint64_t data;
        kernel::TCU::config_mem(1, pe_id(PE::MEM), 0x1000, sizeof(data), TCU::R);
        kernel::TCU::config_recv(2, buf, 5 /* 32 */, 5 /* 32 */, TCU::INVALID_EP, 0, 0);
        kernel::TCU::config_send(3, 0x5678, pe_id(PE::PE0), 2, 5 /* 32 */, 1);

        // here everything still works
        ASSERT_EQ(kernel::TCU::read(1, &data, sizeof(data), 0), Errors::NONE);
        ASSERT_EQ(kernel::TCU::ack_msg(2, buf, reinterpret_cast<const m3::TCU::Message*>(buf)), Errors::NONE);
        ASSERT_EQ(m3::TCU::get().is_valid(3), true);

        ASSERT_EQ(kernel::TCU::invalidate_ep_remote(pe_id(PE::PE0), 1, true), Errors::NONE);
        ASSERT_EQ(kernel::TCU::invalidate_ep_remote(pe_id(PE::PE0), 2, true), Errors::NONE);
        ASSERT_EQ(kernel::TCU::invalidate_ep_remote(pe_id(PE::PE0), 3, true), Errors::NONE);

        // now the EPs are invalid
        ASSERT_EQ(kernel::TCU::read(1, &data, sizeof(data), 0), Errors::NO_MEP);
        ASSERT_EQ(kernel::TCU::ack_msg(2, buf, reinterpret_cast<const m3::TCU::Message*>(buf)), Errors::NO_REP);
        ASSERT_EQ(kernel::TCU::send(3, msg, 0x5678, TCU::NO_REPLIES), Errors::NO_SEP);

        // invalidating again should work as well
        ASSERT_EQ(kernel::TCU::invalidate_ep_remote(pe_id(PE::PE0), 1, true), Errors::NONE);
        ASSERT_EQ(kernel::TCU::invalidate_ep_remote(pe_id(PE::PE0), 2, true), Errors::NONE);
        ASSERT_EQ(kernel::TCU::invalidate_ep_remote(pe_id(PE::PE0), 3, true), Errors::NONE);
    }

    Serial::get() << "non-force send EP invalidation\n";
    {
        kernel::TCU::config_recv(2, buf, 5 /* 32 */, 5 /* 32 */, TCU::INVALID_EP, 0, 0);
        kernel::TCU::config_send(3, 0x5678, pe_id(PE::PE0), 2, 5 /* 32 */, 1);

        // if credits are missing, we can't invalidate it (with force=0)
        ASSERT_EQ(kernel::TCU::send(3, msg, 0x5678, TCU::NO_REPLIES), Errors::NONE);
        ASSERT_EQ(kernel::TCU::invalidate_ep_remote(pe_id(PE::PE0), 3, false), Errors::NO_CREDITS);
        ASSERT_EQ(kernel::TCU::send(3, msg, 0x5678, TCU::NO_REPLIES), Errors::NO_CREDITS);

        // with all credits, we can invalidate
        kernel::TCU::config_send(3, 0x5678, pe_id(PE::PE0), 2, 5 /* 32 */, 1);
        ASSERT_EQ(kernel::TCU::invalidate_ep_remote(pe_id(PE::PE0), 3, false), Errors::NONE);
        ASSERT_EQ(kernel::TCU::send(3, msg, 0x5678, TCU::NO_REPLIES), Errors::NO_SEP);
    }

    Serial::get() << "non-force receive EP invalidation\n";
    {
        kernel::TCU::config_recv(2, buf, 5 /* 32 */, 5 /* 32 */, TCU::INVALID_EP, 0x1, 0x1);

        // invalidation gives us the unread mask
        uint32_t unread;
        ASSERT_EQ(kernel::TCU::invalidate_ep_remote(pe_id(PE::PE0), 2, false, &unread), Errors::NONE);
        ASSERT_EQ(unread, 0x1);

        // EP is invalid
        ASSERT_EQ(kernel::TCU::ack_msg(2, buf, reinterpret_cast<const m3::TCU::Message*>(buf)), Errors::NO_REP);
    }
}

void test_ext() {
    test_inv_ep();
}
