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

#include "common.h"
#include "DTU.h"

// msg size in number of 64-bit elements (max: 100)
#define MSG_SIZE   80
#define REPLY_SIZE 80

static uint64_t msg_list[MSG_SIZE];
static uint64_t reply_list[REPLY_SIZE];

static void test_mem() {
    DTU::config_mem(0, MEM_MODID, 0x1000, sizeof(uint64_t), DTU::RW);
    DTU::config_mem(1, MEM_MODID, 0x1000, sizeof(uint64_t), DTU::R);
    DTU::config_mem(2, MEM_MODID, 0x1000, sizeof(uint64_t), DTU::W);
    DTU::config_mem(3, MEM_MODID, 0x2000, sizeof(uint64_t) * 2, DTU::RW);

    uint64_t data = 1234;

    // test errors
    ASSERT_EQ(DTU::write(0, &data, sizeof(data), 1, 0), Error::INV_ARGS);
    ASSERT_EQ(DTU::write(0, &data, sizeof(data) + 1, 0, 0), Error::INV_ARGS);
    ASSERT_EQ(DTU::write(1, &data, sizeof(data), 0, 0), Error::NO_PERM);
    ASSERT_EQ(DTU::read(2, &data, sizeof(data), 0, 0), Error::NO_PERM);

    // test write + read with offset = 0
    {
        uint64_t data_ctrl = 0;
        ASSERT_EQ(DTU::write(0, &data, sizeof(data), 0, 0), Error::NONE);
        ASSERT_EQ(DTU::read(0, &data_ctrl, sizeof(data), 0, 0), Error::NONE);
        ASSERT_EQ(data, data_ctrl);
    }

    // test write + read with offset != 0
    {
        uint64_t data_ctrl = 0;
        ASSERT_EQ(DTU::write(3, &data, sizeof(data), 4, 0), Error::NONE);
        ASSERT_EQ(DTU::read(3, &data_ctrl, sizeof(data), 4, 0), Error::NONE);
        ASSERT_EQ(data, data_ctrl);
    }
}

static void test_msg(size_t msg_size_in, size_t reply_size_in) {
    constexpr size_t TOTAL_MSG_SIZE = MSG_SIZE * sizeof(uint64_t) + sizeof(DTU::Header);
    constexpr size_t TOTAL_REPLY_SIZE = REPLY_SIZE * sizeof(uint64_t) + sizeof(DTU::Header);
    char buffer[2 * TOTAL_MSG_SIZE];
    char buffer2[2 * TOTAL_REPLY_SIZE];

    DTU::config_recv(1, reinterpret_cast<uintptr_t>(&buffer),  11 /* 2048 */, 10 /* 1024 */, 3);
    DTU::config_recv(2, reinterpret_cast<uintptr_t>(&buffer2), 11 /* 2048 */, 10 /* 1024 */, DTU::NO_REPLIES);

    // send + recv + reply
    {
        DTU::config_send(0, 0x1234, OWN_MODID, 1, 10 /* 1024 */, 1);

        ASSERT_EQ(DTU::send(0, msg_list, msg_size_in * sizeof(msg_list[0]), 0x1111, 2), Error::NONE);

        // fetch message
        const DTU::Message *rmsg;
        while((rmsg = DTU::fetch_msg(1)) == nullptr)
            ;
        // validate contents
        ASSERT_EQ(rmsg->label, 0x1234);
        ASSERT_EQ(rmsg->length, msg_size_in * sizeof(msg_list[0]));
        ASSERT_EQ(rmsg->senderEp, 0);
        ASSERT_EQ(rmsg->replyEp, 2);
        ASSERT_EQ(rmsg->senderPe, OWN_MODID);
        ASSERT_EQ(rmsg->flags, 0);
        const uint64_t *msg_ctrl = reinterpret_cast<const uint64_t*>(rmsg->data);
        for(size_t i = 0; i < msg_size_in; ++i)
            ASSERT_EQ(msg_ctrl[i], msg_list[i]);

        // we need the reply to get our credits back
        ASSERT_EQ(DTU::send(0, msg_list, msg_size_in * sizeof(msg_list[0]), 0, 2), Error::MISS_CREDITS);

        // send reply
        ASSERT_EQ(DTU::reply(1, reply_list, reply_size_in * sizeof(reply_list[0]), rmsg), Error::NONE);

        // fetch reply
        while((rmsg = DTU::fetch_msg(2)) == nullptr)
            ;
        // validate contents
        ASSERT_EQ(rmsg->label, 0x1111);
        ASSERT_EQ(rmsg->length, reply_size_in * sizeof(reply_list[0]));
        ASSERT_EQ(rmsg->senderEp, 1);
        ASSERT_EQ(rmsg->replyEp, 0);
        ASSERT_EQ(rmsg->senderPe, OWN_MODID);
        ASSERT_EQ(rmsg->flags, DTU::Header::FL_REPLY);
        msg_ctrl = reinterpret_cast<const uint64_t*>(rmsg->data);
        for(size_t i = 0; i < reply_size_in; ++i)
            ASSERT_EQ(msg_ctrl[i], reply_list[i]);
        // free slot
        DTU::mark_read(2, rmsg);
    }

    // send + send + recv + recv
    {
        uint64_t msg = 5678;
        uint64_t reply = 9123;

        DTU::config_send(0, 0x1234, OWN_MODID, 1, 6 /* 64 */, 2);

        ASSERT_EQ(DTU::send(0, &msg, sizeof(msg), 0x1111, 2), Error::NONE);
        ASSERT_EQ(DTU::send(0, &msg, sizeof(msg), 0x2222, 2), Error::NONE);
        // we need the reply to get our credits back
        ASSERT_EQ(DTU::send(0, &msg, sizeof(msg), 0, 2), Error::MISS_CREDITS);

        for(int i = 0; i < 2; ++i) {
            // fetch message
            const DTU::Message *rmsg;
            while((rmsg = DTU::fetch_msg(1)) == nullptr)
                ;
            // validate contents
            ASSERT_EQ(rmsg->label, 0x1234);
            ASSERT_EQ(rmsg->length, sizeof(msg));
            ASSERT_EQ(rmsg->senderEp, 0);
            ASSERT_EQ(rmsg->replyEp, 2);
            ASSERT_EQ(rmsg->senderPe, OWN_MODID);
            ASSERT_EQ(rmsg->flags, 0);
            const uint64_t *msg_ctrl = reinterpret_cast<const uint64_t*>(rmsg->data);
            ASSERT_EQ(*msg_ctrl, msg);

            // send reply
            ASSERT_EQ(DTU::reply(1, &reply, sizeof(reply), rmsg), Error::NONE);
        }

        for(int i = 0; i < 2; ++i) {
            // fetch reply
            const DTU::Message *rmsg;
            while((rmsg = DTU::fetch_msg(2)) == nullptr)
                ;
            // validate contents
            ASSERT_EQ(rmsg->label, i == 0 ? 0x1111 : 0x2222);
            ASSERT_EQ(rmsg->length, sizeof(reply));
            ASSERT_EQ(rmsg->senderEp, 1);
            ASSERT_EQ(rmsg->replyEp, 0);
            ASSERT_EQ(rmsg->senderPe, OWN_MODID);
            ASSERT_EQ(rmsg->flags, DTU::Header::FL_REPLY);
            const uint64_t *msg_ctrl = reinterpret_cast<const uint64_t*>(rmsg->data);
            ASSERT_EQ(*msg_ctrl, reply);
            // free slot
            DTU::mark_read(2, rmsg);
        }
    }
}

int main() {
    init();

    // prepare test data
    for(size_t i = 0; i < MSG_SIZE; ++i)
        msg_list[i] = i + 1;
    for(size_t i = 0; i < REPLY_SIZE; ++i)
        reply_list[i] = REPLY_SIZE - i;

    test_mem();

    // test different msg lengths
    test_msg(1, 1);
    test_msg(MSG_SIZE / 4, REPLY_SIZE / 4);
    test_msg(MSG_SIZE / 2, REPLY_SIZE / 2);
    test_msg(MSG_SIZE, REPLY_SIZE);

    deinit();
    return 0;
}
