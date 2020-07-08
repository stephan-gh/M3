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
#include <base/TCU.h>
#include <heap/heap.h>
#include <string.h>

#include "assert.h"
#include "tcuif.h"
#include "pes.h"

// msg size in number of 64-bit elements (max: 100)
#define MSG_SIZE   80

using namespace m3;

static void test_mem_short() {
    kernel::TCU::config_mem(0, pe_id(PE::MEM), 0x1000, sizeof(uint64_t), TCU::R | TCU::W);
    kernel::TCU::config_mem(1, pe_id(PE::MEM), 0x1000, sizeof(uint64_t), TCU::R);
    kernel::TCU::config_mem(2, pe_id(PE::MEM), 0x1000, sizeof(uint64_t), TCU::W);
    kernel::TCU::config_mem(3, pe_id(PE::MEM), 0x2000, sizeof(uint64_t) * 2, TCU::R| TCU::W);
    kernel::TCU::config_send(4, 0x1234, pe_id(PE::PE0), 1, 6 /* 64 */, 2);

    uint64_t data = 1234;

    // test errors
    {
        // not a memory EP
        ASSERT_EQ(kernel::TCU::write(4, &data, sizeof(data), 0), Errors::NO_MEP);
        // offset out of bounds
        ASSERT_EQ(kernel::TCU::write(0, &data, sizeof(data), 1), Errors::OUT_OF_BOUNDS);
        // size out of bounds
        ASSERT_EQ(kernel::TCU::write(0, &data, sizeof(data) + 1, 0), Errors::OUT_OF_BOUNDS);
        // no write permission
        ASSERT_EQ(kernel::TCU::write(1, &data, sizeof(data), 0), Errors::NO_PERM);

        // not a memory EP
        ASSERT_EQ(kernel::TCU::read(4, &data, sizeof(data), 0), Errors::NO_MEP);
        // offset out of bounds
        ASSERT_EQ(kernel::TCU::read(0, &data, sizeof(data), 1), Errors::OUT_OF_BOUNDS);
        // size out of bounds
        ASSERT_EQ(kernel::TCU::read(0, &data, sizeof(data) + 1, 0), Errors::OUT_OF_BOUNDS);
        // no read permission
        ASSERT_EQ(kernel::TCU::read(2, &data, sizeof(data), 0), Errors::NO_PERM);
    }

    // test write + read with offset = 0
    {
        uint64_t data_ctrl = 0;
        ASSERT_EQ(kernel::TCU::write(0, &data, sizeof(data), 0), Errors::NONE);
        ASSERT_EQ(kernel::TCU::read(0, &data_ctrl, sizeof(data), 0), Errors::NONE);
        ASSERT_EQ(data, data_ctrl);
    }

    // test write + read with offset != 0
    {
        uint64_t data_ctrl = 0;
        ASSERT_EQ(kernel::TCU::write(3, &data, sizeof(data), 4), Errors::NONE);
        ASSERT_EQ(kernel::TCU::read(3, &data_ctrl, sizeof(data), 4), Errors::NONE);
        ASSERT_EQ(data, data_ctrl);
    }

    // test 0-byte transfers
    {
        ASSERT_EQ(kernel::TCU::write(3, nullptr, 0, 0), Errors::NONE);
        ASSERT_EQ(kernel::TCU::read(3, nullptr, 0, 0), Errors::NONE);
    }
}


template<typename DATA>
static void test_mem(size_t size_in) {
    DATA buffer[size_in];

    // prepare test data
    DATA msg[size_in];
    for(size_t i = 0; i < size_in; ++i)
        msg[i] = i + 1;

    kernel::TCU::config_mem(0, pe_id(PE::MEM), 0x1000, size_in * sizeof(DATA), TCU::R | TCU::W);

    // test write + read
    {
        ASSERT_EQ(kernel::TCU::write(0, msg, size_in * sizeof(DATA), 0), Errors::NONE);
        ASSERT_EQ(kernel::TCU::read(0, buffer, size_in * sizeof(DATA), 0), Errors::NONE);
        for(size_t i = 0; i < size_in; i++)
            ASSERT_EQ(buffer[i], msg[i]);
    }
}


static void test_msg_short() {
    char buffer[2 * 64];
    char buffer2[2 * 64];
    uintptr_t buf1 = reinterpret_cast<uintptr_t>(&buffer);
    uintptr_t buf2 = reinterpret_cast<uintptr_t>(&buffer2);

    uint64_t msg = 5678;
    uint64_t reply = 9123;

    kernel::TCU::config_recv(1, buf1, 7 /* 128 */, 6 /* 64 */, 3);
    kernel::TCU::config_recv(2, buf2, 7 /* 128 */, 6 /* 64 */, TCU::NO_REPLIES);

    kernel::TCU::config_send(0, 0x1234, pe_id(PE::PE0), 1, 6 /* 64 */, 2);
    kernel::TCU::config_send(5, 0x1234, pe_id(PE::PE0), 1, 6 /* 64 */, TCU::UNLIM_CREDITS);
    kernel::TCU::config_send(6, 0x5678, pe_id(PE::PE0), 1, 4 /* 16 */, 1);

    kernel::TCU::config_recv(7, buf2, 6 /* 64 */, 6 /* 64 */, 8,
                             1 /* make msg 0 (EP 8) occupied */, 0);
    kernel::TCU::config_send(8, 0x5678, pe_id(PE::PE0), 1, 4 /* 16 */, 1);

    // test errors
    {
        // not a send EP
        ASSERT_EQ(kernel::TCU::send(1, &msg, sizeof(msg), 0x1111, 2), Errors::NO_SEP);
        // message too large
        size_t msg_size = 1 + 64 - sizeof(TCU::Message::Header);
        ASSERT_EQ(kernel::TCU::send(0, &msg, msg_size, 0x1111, 2), Errors::OUT_OF_BOUNDS);
        // invalid reply EP
        ASSERT_EQ(kernel::TCU::send(0, &msg, sizeof(msg), 0x1111, 0), Errors::NO_REP);
        // not a receive EP
        ASSERT_EQ(kernel::TCU::ack_msg(0, 0, nullptr), Errors::NO_REP);

        // replying with a normal send EP is not allowed
        const TCU::Message *rmsg = reinterpret_cast<const TCU::Message*>(buf2);
        ASSERT_EQ(kernel::TCU::reply(7, nullptr, 0, buf2, rmsg), Errors::SEND_REPLY_EP);
    }

    // send empty message
    {
        ASSERT_EQ(kernel::TCU::send(6, nullptr, 0, 0x2222, TCU::NO_REPLIES), Errors::NONE);

        // fetch message
        const TCU::Message *rmsg;
        while((rmsg = kernel::TCU::fetch_msg(1, buf1)) == nullptr)
            ;
        // validate contents
        ASSERT_EQ(rmsg->label, 0x5678);
        ASSERT_EQ(rmsg->replylabel, 0x2222);
        ASSERT_EQ(rmsg->length, 0);
        ASSERT_EQ(rmsg->senderEp, 6);
        ASSERT_EQ(rmsg->replySize, 4 /* log2(TCU::Message::Header) */);
        ASSERT_EQ(rmsg->replyEp, TCU::INVALID_EP);
        ASSERT_EQ(rmsg->senderPe, pe_id(PE::PE0));
        ASSERT_EQ(rmsg->flags, 0);

        ASSERT_EQ(kernel::TCU::ack_msg(1, buf1, rmsg), Errors::NONE);
    }

    // send empty message and reply empty message
    {
        ASSERT_EQ(kernel::TCU::credits(0), 2);
        ASSERT_EQ(kernel::TCU::send(0, nullptr, 0, 0x1111, 2), Errors::NONE);
        ASSERT_EQ(kernel::TCU::credits(0), 1);

        // fetch message
        const TCU::Message *rmsg;
        while((rmsg = kernel::TCU::fetch_msg(1, buf1)) == nullptr)
            ;
        // validate contents
        ASSERT_EQ(rmsg->label, 0x1234);
        ASSERT_EQ(rmsg->replylabel, 0x1111);
        ASSERT_EQ(rmsg->length, 0);
        ASSERT_EQ(rmsg->senderEp, 0);
        ASSERT_EQ(rmsg->replySize, 6);
        ASSERT_EQ(rmsg->replyEp, 2);
        ASSERT_EQ(rmsg->senderPe, pe_id(PE::PE0));
        ASSERT_EQ(rmsg->flags, 0);

        // sending with the use-once send EP is not allowed
        epid_t rep = reinterpret_cast<const char*>(rmsg) == buffer ? 3 : 4;
        ASSERT_EQ(kernel::TCU::send(rep, nullptr, 0, 0x1111, TCU::NO_REPLIES), Errors::SEND_REPLY_EP);
        // send empty reply
        ASSERT_EQ(kernel::TCU::reply(1, nullptr, 0, buf1, rmsg), Errors::NONE);

        // fetch reply
        while((rmsg = kernel::TCU::fetch_msg(2, buf2)) == nullptr)
            ;
        // validate contents
        ASSERT_EQ(rmsg->label, 0x1111);
        ASSERT_EQ(rmsg->length, 0);
        ASSERT_EQ(rmsg->senderEp, 1);
        ASSERT_EQ(rmsg->replySize, 0);
        ASSERT_EQ(rmsg->replyEp, 0);
        ASSERT_EQ(rmsg->senderPe, pe_id(PE::PE0));
        ASSERT_EQ(rmsg->flags, TCU::Header::FL_REPLY);
        // free slot
        ASSERT_EQ(kernel::TCU::ack_msg(2, buf2, rmsg), Errors::NONE);
    }

    // send without reply
    {
        ASSERT_EQ(kernel::TCU::credits(0), 2);
        ASSERT_EQ(kernel::TCU::send(0, &msg, sizeof(msg), 0x1111, TCU::NO_REPLIES), Errors::NONE);
        ASSERT_EQ(kernel::TCU::credits(0), 1);

        // fetch message
        const TCU::Message *rmsg;
        while((rmsg = kernel::TCU::fetch_msg(1, buf1)) == nullptr)
            ;
        // validate contents
        ASSERT_EQ(rmsg->label, 0x1234);
        ASSERT_EQ(rmsg->replylabel, 0x1111);
        ASSERT_EQ(rmsg->length, sizeof(msg));
        ASSERT_EQ(rmsg->senderEp, 0);
        ASSERT_EQ(rmsg->replySize, 4 /* log2(TCU::Message::Header) */);
        ASSERT_EQ(rmsg->replyEp, TCU::INVALID_EP);
        ASSERT_EQ(rmsg->senderPe, pe_id(PE::PE0));
        ASSERT_EQ(rmsg->flags, 0);
        const uint64_t *msg_ctrl = reinterpret_cast<const uint64_t*>(rmsg->data);
        ASSERT_EQ(*msg_ctrl, msg);

        // reply with data not allowed
        ASSERT_EQ(kernel::TCU::reply(1, &reply, sizeof(reply), buf1, rmsg), Errors::NO_SEP);
        // empty reply is NOT allowed
        ASSERT_EQ(kernel::TCU::reply(1, nullptr, 0, buf1, rmsg), Errors::NO_SEP);
        ASSERT_EQ(kernel::TCU::ack_msg(1, buf1, rmsg), Errors::NONE);
        // reconfigure EP to get credits "back"
        kernel::TCU::config_send(0, 0x1234, pe_id(PE::PE0), 1, 6 /* 64 */, 2);
    }

    // send + reply without credits
    {
        ASSERT_EQ(kernel::TCU::credits(5), TCU::UNLIM_CREDITS);
        ASSERT_EQ(kernel::TCU::send(5, &msg, sizeof(msg), 0x1111, TCU::NO_REPLIES), Errors::NONE);
        ASSERT_EQ(kernel::TCU::send(5, &msg, sizeof(msg), 0x1111, TCU::NO_REPLIES), Errors::NONE);
        // receive buffer full
        ASSERT_EQ(kernel::TCU::send(5, &msg, sizeof(msg), 0x1111, TCU::NO_REPLIES), Errors::RECV_NO_SPACE);
        // no credits lost
        ASSERT_EQ(kernel::TCU::credits(5), TCU::UNLIM_CREDITS);

        // fetch message
        const TCU::Message *rmsg;
        while((rmsg = kernel::TCU::fetch_msg(1, buf1)) == nullptr)
            ;
        // validate contents
        ASSERT_EQ(rmsg->label, 0x1234);
        ASSERT_EQ(rmsg->replylabel, 0x1111);
        ASSERT_EQ(rmsg->length, sizeof(msg));
        ASSERT_EQ(rmsg->senderEp, TCU::INVALID_EP);
        ASSERT_EQ(rmsg->replySize, 4 /* log2(TCU::Message::Header) */);
        ASSERT_EQ(rmsg->replyEp, TCU::INVALID_EP);
        ASSERT_EQ(rmsg->senderPe, pe_id(PE::PE0));
        ASSERT_EQ(rmsg->flags, 0);
        const uint64_t *msg_ctrl = reinterpret_cast<const uint64_t*>(rmsg->data);
        ASSERT_EQ(*msg_ctrl, msg);

        // reply with data not allowed
        ASSERT_EQ(kernel::TCU::reply(1, &reply, sizeof(reply), buf1, rmsg), Errors::NO_SEP);
        // empty reply is NOT allowed
        ASSERT_EQ(kernel::TCU::reply(1, nullptr, 0, buf1, rmsg), Errors::NO_SEP);
        ASSERT_EQ(kernel::TCU::ack_msg(1, buf1, rmsg), Errors::NONE);
        // credits are still the same
        ASSERT_EQ(kernel::TCU::credits(5), TCU::UNLIM_CREDITS);

        // ack the other message we sent above
        rmsg = kernel::TCU::fetch_msg(1, buf1);
        ASSERT(rmsg != nullptr);
        ASSERT_EQ(kernel::TCU::ack_msg(1, buf1, rmsg), Errors::NONE);
    }

    // send + send + recv + recv
    {
        ASSERT_EQ(kernel::TCU::send(0, &msg, sizeof(msg), 0x1111, 2), Errors::NONE);
        ASSERT_EQ(kernel::TCU::send(0, &msg, sizeof(msg), 0x2222, 2), Errors::NONE);
        // we need the reply to get our credits back
        ASSERT_EQ(kernel::TCU::send(0, &msg, sizeof(msg), 0, 2), Errors::NO_CREDITS);

        for(int i = 0; i < 2; ++i) {
            // fetch message
            const TCU::Message *rmsg;
            while((rmsg = kernel::TCU::fetch_msg(1, buf1)) == nullptr)
                ;
            // validate contents
            ASSERT_EQ(rmsg->label, 0x1234);
            ASSERT_EQ(rmsg->replylabel, i == 0 ? 0x1111 : 0x2222);
            ASSERT_EQ(rmsg->length, sizeof(msg));
            ASSERT_EQ(rmsg->senderEp, 0);
            ASSERT_EQ(rmsg->replySize, 6);
            ASSERT_EQ(rmsg->replyEp, 2);
            ASSERT_EQ(rmsg->senderPe, pe_id(PE::PE0));
            ASSERT_EQ(rmsg->flags, 0);
            const uint64_t *msg_ctrl = reinterpret_cast<const uint64_t*>(rmsg->data);
            ASSERT_EQ(*msg_ctrl, msg);

            // message too large
            size_t msg_size = 1 + 64 - sizeof(TCU::Message::Header);
            ASSERT_EQ(kernel::TCU::reply(1, &reply, msg_size, buf1, rmsg), Errors::OUT_OF_BOUNDS);
            // send reply
            ASSERT_EQ(kernel::TCU::reply(1, &reply, sizeof(reply), buf1, rmsg), Errors::NONE);
        }

        for(int i = 0; i < 2; ++i) {
            // fetch reply
            const TCU::Message *rmsg;
            while((rmsg = kernel::TCU::fetch_msg(2, buf2)) == nullptr)
                ;
            // validate contents
            ASSERT_EQ(rmsg->label, i == 0 ? 0x1111 : 0x2222);
            ASSERT_EQ(rmsg->length, sizeof(reply));
            ASSERT_EQ(rmsg->senderEp, 1);
            ASSERT_EQ(rmsg->replySize, 0);
            ASSERT_EQ(rmsg->replyEp, 0);
            ASSERT_EQ(rmsg->senderPe, pe_id(PE::PE0));
            ASSERT_EQ(rmsg->flags, TCU::Header::FL_REPLY);
            const uint64_t *msg_ctrl = reinterpret_cast<const uint64_t*>(rmsg->data);
            ASSERT_EQ(*msg_ctrl, reply);
            // free slot
            ASSERT_EQ(kernel::TCU::ack_msg(2, buf2, rmsg), Errors::NONE);
        }
    }
}

template<typename DATA>
static void test_msg(size_t msg_size_in, size_t reply_size_in) {
    const size_t TOTAL_MSG_SIZE = msg_size_in * sizeof(DATA) + sizeof(TCU::Header);
    const size_t TOTAL_REPLY_SIZE = reply_size_in * sizeof(DATA) + sizeof(TCU::Header);

    char rbuffer[2 * TOTAL_MSG_SIZE];
    char rbuffer2[2 * TOTAL_REPLY_SIZE];
    uintptr_t buf1 = reinterpret_cast<uintptr_t>(&rbuffer);
    uintptr_t buf2 = reinterpret_cast<uintptr_t>(&rbuffer2);

    // prepare test data
    DATA msg[msg_size_in];
    DATA reply[reply_size_in];
    for(size_t i = 0; i < msg_size_in; ++i)
        msg[i] = i + 1;
    for(size_t i = 0; i < reply_size_in; ++i)
        reply[i] = reply_size_in - i;

    TCU::reg_t slot_msgsize = m3::getnextlog2(TOTAL_MSG_SIZE);
    TCU::reg_t slot_replysize = m3::getnextlog2(TOTAL_REPLY_SIZE);

    kernel::TCU::config_recv(1, buf1, slot_msgsize+1, slot_msgsize, 3);
    kernel::TCU::config_recv(2, buf2, slot_replysize+1, slot_replysize, TCU::NO_REPLIES);

    // send + recv + reply
    {
        kernel::TCU::config_send(0, 0x1234, pe_id(PE::PE0), 1, slot_msgsize, 1);

        ASSERT_EQ(kernel::TCU::send(0, msg, msg_size_in * sizeof(DATA), 0x1111, 2), Errors::NONE);

        // fetch message
        const TCU::Message *rmsg;
        while((rmsg = kernel::TCU::fetch_msg(1, buf1)) == nullptr)
            ;
        // validate contents
        ASSERT_EQ(rmsg->label, 0x1234);
        ASSERT_EQ(rmsg->replylabel, 0x1111);
        ASSERT_EQ(rmsg->length, msg_size_in * sizeof(DATA));
        ASSERT_EQ(rmsg->senderEp, 0);
        ASSERT_EQ(rmsg->replyEp, 2);
        ASSERT_EQ(rmsg->senderPe, pe_id(PE::PE0));
        ASSERT_EQ(rmsg->flags, 0);
        const DATA *msg_ctrl = reinterpret_cast<const DATA*>(rmsg->data);
        for(size_t i = 0; i < msg_size_in; ++i)
            ASSERT_EQ(msg_ctrl[i], msg[i]);

        // we need the reply to get our credits back
        ASSERT_EQ(kernel::TCU::send(0, msg, msg_size_in * sizeof(DATA), 0, 2), Errors::NO_CREDITS);

        // send reply
        ASSERT_EQ(kernel::TCU::reply(1, reply, reply_size_in * sizeof(DATA), buf1, rmsg), Errors::NONE);

        // fetch reply
        while((rmsg = kernel::TCU::fetch_msg(2, buf2)) == nullptr)
            ;
        // validate contents
        ASSERT_EQ(rmsg->label, 0x1111);
        ASSERT_EQ(rmsg->length, reply_size_in * sizeof(DATA));
        ASSERT_EQ(rmsg->senderEp, 1);
        ASSERT_EQ(rmsg->replyEp, 0);
        ASSERT_EQ(rmsg->senderPe, pe_id(PE::PE0));
        ASSERT_EQ(rmsg->flags, TCU::Header::FL_REPLY);
        msg_ctrl = reinterpret_cast<const DATA*>(rmsg->data);
        for(size_t i = 0; i < reply_size_in; ++i)
            ASSERT_EQ(msg_ctrl[i], reply[i]);
        // free slot
        ASSERT_EQ(kernel::TCU::ack_msg(2, buf2, rmsg), Errors::NONE);
    }
}

int main() {
    Serial::get() << "Starting TCU tests\n";

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

    Serial::get() << "\x1B[1;32mAll tests successful!\x1B[0;m\n";
    return 0;
}
