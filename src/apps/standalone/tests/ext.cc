/*
 * Copyright (C) 2021-2022 Nils Asmussen, Barkhausen Institut
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

static constexpr epid_t MEP = TCU::FIRST_USER_EP;
static constexpr epid_t SEP = TCU::FIRST_USER_EP + 1;
static constexpr epid_t REP = TCU::FIRST_USER_EP + 2;

static void test_inv_ep() {
    char rbuffer[32];
    uintptr_t buf = reinterpret_cast<uintptr_t>(&rbuffer);

    MsgBuf msg;
    msg.cast<uint64_t>() = 0xDEADBEEF;

    Serial::get() << "force invalidation\n";
    {
        uint64_t data;
        kernel::TCU::config_mem(MEP, tile_id(Tile::MEM), 0x1000, sizeof(data), TCU::R);
        kernel::TCU::config_recv(REP, buf, 5 /* 32 */, 5 /* 32 */, TCU::INVALID_EP, 0, 0);
        kernel::TCU::config_send(SEP, 0x5678, tile_id(Tile::T0), REP, 5 /* 32 */, 1);

        // here everything still works
        ASSERT_EQ(kernel::TCU::read(MEP, &data, sizeof(data), 0), Errors::NONE);
        ASSERT_EQ(kernel::TCU::ack_msg(REP, buf, reinterpret_cast<const m3::TCU::Message*>(buf)), Errors::NONE);
        ASSERT_EQ(m3::TCU::get().is_valid(SEP), true);

        ASSERT_EQ(kernel::TCU::invalidate_ep_remote(tile_id(Tile::T0), MEP, true), Errors::NONE);
        ASSERT_EQ(kernel::TCU::invalidate_ep_remote(tile_id(Tile::T0), SEP, true), Errors::NONE);
        ASSERT_EQ(kernel::TCU::invalidate_ep_remote(tile_id(Tile::T0), REP, true), Errors::NONE);

        // now the EPs are invalid
        ASSERT_EQ(kernel::TCU::read(MEP, &data, sizeof(data), 0), Errors::NO_MEP);
        ASSERT_EQ(kernel::TCU::ack_msg(REP, buf, reinterpret_cast<const m3::TCU::Message*>(buf)), Errors::NO_REP);
        ASSERT_EQ(kernel::TCU::send(SEP, msg, 0x5678, TCU::NO_REPLIES), Errors::NO_SEP);

        // invalidating again should work as well
        ASSERT_EQ(kernel::TCU::invalidate_ep_remote(tile_id(Tile::T0), MEP, true), Errors::NONE);
        ASSERT_EQ(kernel::TCU::invalidate_ep_remote(tile_id(Tile::T0), SEP, true), Errors::NONE);
        ASSERT_EQ(kernel::TCU::invalidate_ep_remote(tile_id(Tile::T0), REP, true), Errors::NONE);
    }

    Serial::get() << "non-force send EP invalidation\n";
    {
        kernel::TCU::config_recv(REP, buf, 5 /* 32 */, 5 /* 32 */, TCU::INVALID_EP, 0, 0);
        kernel::TCU::config_send(SEP, 0x5678, tile_id(Tile::T0), REP, 5 /* 32 */, 1);

        // if credits are missing, we can't invalidate it (with force=0)
        ASSERT_EQ(kernel::TCU::send(SEP, msg, 0x5678, TCU::NO_REPLIES), Errors::NONE);
        ASSERT_EQ(kernel::TCU::invalidate_ep_remote(tile_id(Tile::T0), SEP, false), Errors::NO_CREDITS);
        ASSERT_EQ(kernel::TCU::send(SEP, msg, 0x5678, TCU::NO_REPLIES), Errors::NO_CREDITS);

        // with all credits, we can invalidate
        kernel::TCU::config_send(SEP, 0x5678, tile_id(Tile::T0), 2, 5 /* 32 */, 1);
        ASSERT_EQ(kernel::TCU::invalidate_ep_remote(tile_id(Tile::T0), SEP, false), Errors::NONE);
        ASSERT_EQ(kernel::TCU::send(SEP, msg, 0x5678, TCU::NO_REPLIES), Errors::NO_SEP);
    }

    Serial::get() << "non-force receive EP invalidation\n";
    {
        kernel::TCU::config_recv(REP, buf, 5 /* 32 */, 5 /* 32 */, TCU::INVALID_EP, 0x1, 0x1);

        // invalidation gives us the unread mask
        uint32_t unread;
        ASSERT_EQ(kernel::TCU::invalidate_ep_remote(tile_id(Tile::T0), REP, false, &unread), Errors::NONE);
        ASSERT_EQ(unread, 0x1);

        // EP is invalid
        ASSERT_EQ(kernel::TCU::ack_msg(REP, buf, reinterpret_cast<const m3::TCU::Message*>(buf)), Errors::NO_REP);
    }
}

void test_ext() {
    test_inv_ep();
}
