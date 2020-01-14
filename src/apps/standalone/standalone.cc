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


// compute log to base 2 and round down
static reg_t cLog2(size_t size) {
    reg_t tmp_log = 0;
    while (size > 1) {
        size >>= 1;
        tmp_log++;
    }
    return tmp_log;
}


static void test_mem_short() {
    DTU::config_mem(0, MEM_MODID, 0x1000, sizeof(uint64_t), DTU::RW);
    DTU::config_mem(1, MEM_MODID, 0x1000, sizeof(uint64_t), DTU::R);
    DTU::config_mem(2, MEM_MODID, 0x1000, sizeof(uint64_t), DTU::W);
    DTU::config_mem(3, MEM_MODID, 0x2000, sizeof(uint64_t) * 2, DTU::RW);
    DTU::config_send(4, 0x1234, OWN_MODID, 1, 6 /* 64 */, 2);

    uint64_t data = 1234;

    // test errors
    {
        // not a memory EP
        ASSERT_EQ(DTU::write(4, &data, sizeof(data), 0, 0), Error::INV_EP);
        // offset out of bounds
        ASSERT_EQ(DTU::write(0, &data, sizeof(data), 1, 0), Error::INV_ARGS);
        // size out of bounds
        ASSERT_EQ(DTU::write(0, &data, sizeof(data) + 1, 0, 0), Error::INV_ARGS);
        // no write permission
        ASSERT_EQ(DTU::write(1, &data, sizeof(data), 0, 0), Error::NO_PERM);

        // not a memory EP
        ASSERT_EQ(DTU::read(4, &data, sizeof(data), 0, 0), Error::INV_EP);
        // offset out of bounds
        ASSERT_EQ(DTU::read(0, &data, sizeof(data), 1, 0), Error::INV_ARGS);
        // size out of bounds
        ASSERT_EQ(DTU::read(0, &data, sizeof(data) + 1, 0, 0), Error::INV_ARGS);
        // no read permission
        ASSERT_EQ(DTU::read(2, &data, sizeof(data), 0, 0), Error::NO_PERM);
    }

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


template<typename DATA>
static void test_mem(size_t size_in) {
    DATA buffer[size_in];

    // prepare test data
    DATA msg[size_in];
    for(size_t i = 0; i < size_in; ++i)
        msg[i] = i + 1;

    DTU::config_mem(0, MEM_MODID, 0x1000, size_in * sizeof(DATA), DTU::RW);

    // test write + read
    {
        ASSERT_EQ(DTU::write(0, msg, size_in * sizeof(DATA), 0, 0), Error::NONE);
        ASSERT_EQ(DTU::read(0, buffer, size_in * sizeof(DATA), 0, 0), Error::NONE);
        for(size_t i = 0; i < size_in; i++)
            ASSERT_EQ(buffer[i], msg[i]);
    }
}


static void test_msg_short() {
    char buffer[2 * 64];
    char buffer2[2 * 64];

    uint64_t msg = 5678;
    uint64_t reply = 9123;

    DTU::config_recv(1, reinterpret_cast<uintptr_t>(&buffer), 7 /* 128 */, 6 /* 64 */, 3);
    DTU::config_recv(2, reinterpret_cast<uintptr_t>(&buffer2), 7 /* 128 */, 6 /* 64 */, DTU::NO_REPLIES);

    DTU::config_send(0, 0x1234, OWN_MODID, 1, 6 /* 64 */, 2);
    DTU::config_send(5, 0x1234, OWN_MODID, 1, 6 /* 64 */, 0x3F);

    // test errors
    {
        // not a send EP
        ASSERT_EQ(DTU::send(1, &msg, sizeof(msg), 0x1111, 2), Error::INV_EP);
        // message too large
        ASSERT_EQ(DTU::send(0, &msg, 1 + 64 - sizeof(DTU::Message::Header), 0x1111, 2), Error::INV_ARGS);
        // invalid reply EP
        ASSERT_EQ(DTU::send(0, &msg, sizeof(msg), 0x1111, 0), Error::INV_EP);
        // not a reply EP
        ASSERT_EQ(DTU::mark_read(0, nullptr), Error::INV_EP);
    }

    // send without reply
    {
        ASSERT_EQ(DTU::credits(0), 2);
        ASSERT_EQ(DTU::send(0, &msg, sizeof(msg), 0x1111, DTU::NO_REPLIES), Error::NONE);
        ASSERT_EQ(DTU::credits(0), 1);

        // fetch message
        const DTU::Message *rmsg;
        while((rmsg = DTU::fetch_msg(1)) == nullptr)
            ;
        // validate contents
        ASSERT_EQ(rmsg->label, 0x1234);
        ASSERT_EQ(rmsg->replylabel, 0x1111);
        ASSERT_EQ(rmsg->length, sizeof(msg));
        ASSERT_EQ(rmsg->senderEp, 0);
        ASSERT_EQ(rmsg->replySize, 4 /* log2(DTU::Message::Header) */);
        ASSERT_EQ(rmsg->replyEp, DTU::INVALID_EP);
        ASSERT_EQ(rmsg->senderPe, OWN_MODID);
        ASSERT_EQ(rmsg->flags, 0);
        const uint64_t *msg_ctrl = reinterpret_cast<const uint64_t*>(rmsg->data);
        ASSERT_EQ(*msg_ctrl, msg);

        // reply with data not allowed
        ASSERT_EQ(DTU::reply(1, &reply, sizeof(reply), rmsg), Error::INV_ARGS);
        // sending with the use-once send EP is not allowed
        ASSERT_EQ(DTU::send(3, nullptr, 0, 0x1111, DTU::NO_REPLIES), Error::INV_EP);
        // empty reply is allowed
        ASSERT_EQ(DTU::reply(1, nullptr, 0, rmsg), Error::NONE);
        // credits are back now
        ASSERT_EQ(DTU::credits(0), 2);
    }

    // send + reply without credits
    {
        ASSERT_EQ(DTU::credits(5), 0x3F);
        ASSERT_EQ(DTU::send(5, &msg, sizeof(msg), 0x1111, DTU::NO_REPLIES), Error::NONE);
        ASSERT_EQ(DTU::send(5, &msg, sizeof(msg), 0x1111, DTU::NO_REPLIES), Error::NONE);
        // receive buffer full
        ASSERT_EQ(DTU::send(5, &msg, sizeof(msg), 0x1111, DTU::NO_REPLIES), Error::NO_RING_SPACE);
        // no credits lost
        ASSERT_EQ(DTU::credits(5), 0x3F);

        // fetch message
        const DTU::Message *rmsg;
        while((rmsg = DTU::fetch_msg(1)) == nullptr)
            ;
        // validate contents
        ASSERT_EQ(rmsg->label, 0x1234);
        ASSERT_EQ(rmsg->replylabel, 0x1111);
        ASSERT_EQ(rmsg->length, sizeof(msg));
        ASSERT_EQ(rmsg->senderEp, DTU::INVALID_EP);
        ASSERT_EQ(rmsg->replySize, 4 /* log2(DTU::Message::Header) */);
        ASSERT_EQ(rmsg->replyEp, DTU::INVALID_EP);
        ASSERT_EQ(rmsg->senderPe, OWN_MODID);
        ASSERT_EQ(rmsg->flags, 0);
        const uint64_t *msg_ctrl = reinterpret_cast<const uint64_t*>(rmsg->data);
        ASSERT_EQ(*msg_ctrl, msg);

        // reply with data not allowed
        ASSERT_EQ(DTU::reply(1, &reply, sizeof(reply), rmsg), Error::INV_ARGS);
        // empty reply is allowed
        ASSERT_EQ(DTU::reply(1, nullptr, 0, rmsg), Error::NONE);
        // credits are still the same
        ASSERT_EQ(DTU::credits(5), 0x3F);

        // ack the other message we sent above
        rmsg = DTU::fetch_msg(1);
        ASSERT(rmsg != nullptr);
        ASSERT_EQ(DTU::mark_read(1, rmsg), Error::NONE);
    }

    // send + send + recv + recv
    {
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
            ASSERT_EQ(rmsg->replylabel, i == 0 ? 0x1111 : 0x2222);
            ASSERT_EQ(rmsg->length, sizeof(msg));
            ASSERT_EQ(rmsg->senderEp, 0);
            ASSERT_EQ(rmsg->replySize, 6);
            ASSERT_EQ(rmsg->replyEp, 2);
            ASSERT_EQ(rmsg->senderPe, OWN_MODID);
            ASSERT_EQ(rmsg->flags, 0);
            const uint64_t *msg_ctrl = reinterpret_cast<const uint64_t*>(rmsg->data);
            ASSERT_EQ(*msg_ctrl, msg);

            // message too large
            ASSERT_EQ(DTU::reply(1, &reply, 1 + 64 - sizeof(DTU::Message::Header), rmsg), Error::INV_ARGS);
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
            ASSERT_EQ(rmsg->replySize, 0);
            ASSERT_EQ(rmsg->replyEp, 0);
            ASSERT_EQ(rmsg->senderPe, OWN_MODID);
            ASSERT_EQ(rmsg->flags, DTU::Header::FL_REPLY);
            const uint64_t *msg_ctrl = reinterpret_cast<const uint64_t*>(rmsg->data);
            ASSERT_EQ(*msg_ctrl, reply);
            // free slot
            ASSERT_EQ(DTU::mark_read(2, rmsg), Error::NONE);
        }
    }
}

template<typename DATA>
static void test_msg(size_t msg_size_in, size_t reply_size_in) {
    const size_t TOTAL_MSG_SIZE = msg_size_in * sizeof(DATA) + sizeof(DTU::Header);
    const size_t TOTAL_REPLY_SIZE = reply_size_in * sizeof(DATA) + sizeof(DTU::Header);

    char rbuffer[2 * TOTAL_MSG_SIZE];
    char rbuffer2[2 * TOTAL_REPLY_SIZE];

    // prepare test data
    DATA msg[msg_size_in];
    DATA reply[reply_size_in];
    for(size_t i = 0; i < msg_size_in; ++i)
        msg[i] = i + 1;
    for(size_t i = 0; i < reply_size_in; ++i)
        reply[i] = reply_size_in - i;

    reg_t slot_msgsize = cLog2(TOTAL_MSG_SIZE) + 1;
    reg_t slot_replysize = cLog2(TOTAL_REPLY_SIZE) + 1;

    DTU::config_recv(1, reinterpret_cast<uintptr_t>(&rbuffer), slot_msgsize+1, slot_msgsize, 3);
    DTU::config_recv(2, reinterpret_cast<uintptr_t>(&rbuffer2), slot_replysize+1, slot_replysize, DTU::NO_REPLIES);

    // send + recv + reply
    {
        DTU::config_send(0, 0x1234, OWN_MODID, 1, slot_msgsize, 1);

        ASSERT_EQ(DTU::send(0, msg, msg_size_in * sizeof(DATA), 0x1111, 2), Error::NONE);

        // fetch message
        const DTU::Message *rmsg;
        while((rmsg = DTU::fetch_msg(1)) == nullptr)
            ;
        // validate contents
        ASSERT_EQ(rmsg->label, 0x1234);
        ASSERT_EQ(rmsg->replylabel, 0x1111);
        ASSERT_EQ(rmsg->length, msg_size_in * sizeof(DATA));
        ASSERT_EQ(rmsg->senderEp, 0);
        ASSERT_EQ(rmsg->replyEp, 2);
        ASSERT_EQ(rmsg->senderPe, OWN_MODID);
        ASSERT_EQ(rmsg->flags, 0);
        const DATA *msg_ctrl = reinterpret_cast<const DATA*>(rmsg->data);
        for(size_t i = 0; i < msg_size_in; ++i)
            ASSERT_EQ(msg_ctrl[i], msg[i]);

        // we need the reply to get our credits back
        ASSERT_EQ(DTU::send(0, msg, msg_size_in * sizeof(DATA), 0, 2), Error::MISS_CREDITS);

        // send reply
        ASSERT_EQ(DTU::reply(1, reply, reply_size_in * sizeof(DATA), rmsg), Error::NONE);

        // fetch reply
        while((rmsg = DTU::fetch_msg(2)) == nullptr)
            ;
        // validate contents
        ASSERT_EQ(rmsg->label, 0x1111);
        ASSERT_EQ(rmsg->length, reply_size_in * sizeof(DATA));
        ASSERT_EQ(rmsg->senderEp, 1);
        ASSERT_EQ(rmsg->replyEp, 0);
        ASSERT_EQ(rmsg->senderPe, OWN_MODID);
        ASSERT_EQ(rmsg->flags, DTU::Header::FL_REPLY);
        msg_ctrl = reinterpret_cast<const DATA*>(rmsg->data);
        for(size_t i = 0; i < reply_size_in; ++i)
            ASSERT_EQ(msg_ctrl[i], reply[i]);
        // free slot
        ASSERT_EQ(DTU::mark_read(2, rmsg), Error::NONE);
    }
}


int main() {
    init();

    test_mem_short();
    test_msg_short();

    // test different lengths
    for(size_t i = 1; i <= MSG_SIZE; i++) {
        test_mem<uint8_t>(i);
        test_mem<uint16_t>(i);
        test_mem<uint32_t>(i);
        test_mem<uint64_t>(i);

        test_msg<uint8_t>(i, i);
        test_msg<uint16_t>(i, i);
        test_msg<uint32_t>(i, i);
        test_msg<uint64_t>(i, i);
    }

    deinit();
    return 0;
}
