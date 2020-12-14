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
    uint64_t data = 1234;

    ASSERT_EQ(kernel::TCU::unknown_cmd(), Errors::UNKNOWN_CMD);

    kernel::TCU::config_mem(1, pe_id(PE::MEM), 0x1000, sizeof(uint64_t), TCU::R | TCU::W);

    // test write
    {
        kernel::TCU::config_mem(2, pe_id(PE::MEM), 0x1000, sizeof(uint64_t), TCU::R);
        kernel::TCU::config_send(3, 0x1234, pe_id(PE::PE0), 1, 6 /* 64 */, 2);

        // not a memory EP
        ASSERT_EQ(kernel::TCU::write(3, &data, sizeof(data), 0), Errors::NO_MEP);
        // offset out of bounds
        ASSERT_EQ(kernel::TCU::write(1, &data, sizeof(data), 1), Errors::OUT_OF_BOUNDS);
        // size out of bounds
        ASSERT_EQ(kernel::TCU::write(1, &data, sizeof(data) + 1, 0), Errors::OUT_OF_BOUNDS);
        // no write permission
        ASSERT_EQ(kernel::TCU::write(2, &data, sizeof(data), 0), Errors::NO_PERM);
    }

    // test read
    {
        kernel::TCU::config_mem(2, pe_id(PE::MEM), 0x1000, sizeof(uint64_t), TCU::W);
        kernel::TCU::config_send(3, 0x1234, pe_id(PE::PE0), 1, 6 /* 64 */, 2);

        // not a memory EP
        ASSERT_EQ(kernel::TCU::read(3, &data, sizeof(data), 0), Errors::NO_MEP);
        // offset out of bounds
        ASSERT_EQ(kernel::TCU::read(1, &data, sizeof(data), 1), Errors::OUT_OF_BOUNDS);
        // size out of bounds
        ASSERT_EQ(kernel::TCU::read(1, &data, sizeof(data) + 1, 0), Errors::OUT_OF_BOUNDS);
        // no read permission
        ASSERT_EQ(kernel::TCU::read(2, &data, sizeof(data), 0), Errors::NO_PERM);
    }

    // test write + read with offset = 0
    {
        uint64_t data_ctrl = 0;
        ASSERT_EQ(kernel::TCU::write(1, &data, sizeof(data), 0), Errors::NONE);
        ASSERT_EQ(kernel::TCU::read(1, &data_ctrl, sizeof(data), 0), Errors::NONE);
        ASSERT_EQ(data, data_ctrl);
    }

    // test write + read with offset != 0
    {
        kernel::TCU::config_mem(2, pe_id(PE::MEM), 0x2000, sizeof(uint64_t) * 2, TCU::R| TCU::W);

        uint64_t data_ctrl = 0;
        ASSERT_EQ(kernel::TCU::write(2, &data, sizeof(data), 4), Errors::NONE);
        ASSERT_EQ(kernel::TCU::read(2, &data_ctrl, sizeof(data), 4), Errors::NONE);
        ASSERT_EQ(data, data_ctrl);
    }

    // test 0-byte transfers
    {
        kernel::TCU::config_mem(2, pe_id(PE::MEM), 0x2000, sizeof(uint64_t) * 2, TCU::R| TCU::W);

        ASSERT_EQ(kernel::TCU::write(2, nullptr, 0, 0), Errors::NONE);
        ASSERT_EQ(kernel::TCU::read(2, nullptr, 0, 0), Errors::NONE);
    }
}

static uint8_t src_buf[16384];
static uint8_t dst_buf[16384];
static uint8_t mem_buf[16384];

static void test_mem_large(PE mem_pe) {
    for(size_t i = 0; i < ARRAY_SIZE(src_buf); ++i)
        src_buf[i] = i;

    size_t addr = mem_pe == PE::MEM ? 0x1000 : reinterpret_cast<size_t>(mem_buf);
    kernel::TCU::config_mem(1, pe_id(mem_pe), addr, sizeof(src_buf), TCU::R | TCU::W);

    const size_t sizes[] = {64, 128, 256, 512, 1024, 2048, 4096, 8192, 16384};
    for(auto size : sizes) {
        Serial::get() << "READ+WRITE with " << size << " bytes with PE" << (int)mem_pe << "\n";

        ASSERT_EQ(kernel::TCU::write(1, src_buf, size, 0), Errors::NONE);
        ASSERT_EQ(kernel::TCU::read(1, dst_buf, size, 0), Errors::NONE);
        for(size_t i = 0; i < size; ++i)
            ASSERT_EQ(src_buf[i], dst_buf[i]);
    }
}

static void test_mem_rdwr(PE mem_pe) {
    for(size_t i = 0; i < ARRAY_SIZE(src_buf); ++i)
        src_buf[i] = i;

    size_t addr = mem_pe == PE::MEM ? 0x1000 : reinterpret_cast<size_t>(mem_buf);
    kernel::TCU::config_mem(1, pe_id(mem_pe), addr, sizeof(src_buf), TCU::R | TCU::W);

    const size_t sizes[] = {4096, 8192};
    for(auto size : sizes) {
        memset(dst_buf, 0, sizeof(dst_buf));

        Serial::get() << "READ+WRITE+READ+WRITE with " << size << " bytes with PE" << (int)mem_pe << "\n";

        // first write our data
        ASSERT_EQ(kernel::TCU::write(1, src_buf, size, 0), Errors::NONE);
        // read it into a buffer for the next write
        ASSERT_EQ(kernel::TCU::read(1, dst_buf, size, 0), Errors::NONE);
        // write the just read data
        ASSERT_EQ(kernel::TCU::write(1, dst_buf, size, 0), Errors::NONE);
        // read it again for checking purposes
        ASSERT_EQ(kernel::TCU::read(1, dst_buf, size, 0), Errors::NONE);
        for(size_t i = 0; i < size; ++i)
            ASSERT_EQ(src_buf[i], dst_buf[i]);
    }
}

template<typename DATA>
static void test_mem(size_t size_in) {
    Serial::get() << "READ+WRITE with " << size_in << " " << sizeof(DATA) << "B words\n";

    DATA buffer[size_in];

    // prepare test data
    DATA msg[size_in];
    for(size_t i = 0; i < size_in; ++i)
        msg[i] = i + 1;

    kernel::TCU::config_mem(1, pe_id(PE::MEM), 0x1000, size_in * sizeof(DATA), TCU::R | TCU::W);

    // test write + read
    ASSERT_EQ(kernel::TCU::write(1, msg, size_in * sizeof(DATA), 0), Errors::NONE);
    ASSERT_EQ(kernel::TCU::read(1, buffer, size_in * sizeof(DATA), 0), Errors::NONE);
    for(size_t i = 0; i < size_in; i++)
        ASSERT_EQ(buffer[i], msg[i]);
}

static void test_msg_errors() {
    ALIGNED(8) char buffer[2 * 64];
    uintptr_t buf1 = reinterpret_cast<uintptr_t>(&buffer);

    uint64_t msg = 5678;

    // not a send EP
    {
        kernel::TCU::config_recv(1, buf1, 6 /* 64 */, 6 /* 64 */, 3);
        ASSERT_EQ(kernel::TCU::send(1, &msg, sizeof(msg), 0x1111, TCU::NO_REPLIES), Errors::NO_SEP);
    }

    {
        kernel::TCU::config_send(1, 0x1234, pe_id(PE::PE0), 1, 6 /* 64 */, 2);

        // message too large
        size_t msg_size = 1 + 64 - sizeof(TCU::Message::Header);
        ASSERT_EQ(kernel::TCU::send(1, &msg, msg_size, 0x1111, TCU::NO_REPLIES), Errors::OUT_OF_BOUNDS);
        // invalid reply EP
        ASSERT_EQ(kernel::TCU::send(1, &msg, sizeof(msg), 0x1111, 1), Errors::NO_REP);
        // not a receive EP
        ASSERT_EQ(kernel::TCU::ack_msg(1, 0, nullptr), Errors::NO_REP);
    }

    {
        kernel::TCU::config_recv(1, buf1, 6 /* 64 */, 6 /* 64 */, 3);

        // reply on message that's out of bounds
        const TCU::Message *rmsg = reinterpret_cast<const TCU::Message*>(buf1 + (1 << 6));
        ASSERT_EQ(kernel::TCU::reply(1, nullptr, 0, buf1, rmsg), Errors::INV_MSG_OFF);
        // ack message that's out of bounds
        ASSERT_EQ(kernel::TCU::ack_msg(1, buf1, rmsg), Errors::INV_MSG_OFF);
    }

    // no replies allowed for this receive EP
    {
        kernel::TCU::config_recv(1, buf1, 6 /* 64 */, 6 /* 64 */, TCU::NO_REPLIES);
        const TCU::Message *rmsg = reinterpret_cast<const TCU::Message*>(buf1);
        ASSERT_EQ(kernel::TCU::reply(1, nullptr, 0, buf1, rmsg), Errors::REPLIES_DISABLED);
    }

    // replying with a normal send EP is not allowed
    {
        kernel::TCU::config_recv(1, buf1, 6 /* 64 */, 6 /* 64 */, 2,
                                 1 /* make msg 0 (EP 2) occupied */, 0);
        kernel::TCU::config_send(2, 0x5678, pe_id(PE::PE0), 1, 4 /* 16 */, 1);
        const TCU::Message *rmsg = reinterpret_cast<const TCU::Message*>(buf1);
        ASSERT_EQ(kernel::TCU::reply(1, nullptr, 0, buf1, rmsg), Errors::SEND_REPLY_EP);
    }

    // receive EP invalid
    {
        kernel::TCU::config_invalid(2);
        kernel::TCU::config_send(1, 0x5678, pe_id(PE::PE0), 2 /* invalid REP */, 4 /* 16 */, 1);
        ASSERT_EQ(kernel::TCU::send(1, nullptr, 0, 0x1111, TCU::NO_REPLIES), Errors::RECV_GONE);
    }

    // receive buffer misaligned
    {
        kernel::TCU::config_recv(1, buf1 + 1 /* misaligned */, 6 /* 64 */, 6 /* 64 */, TCU::NO_REPLIES);
        kernel::TCU::config_send(2, 0x5678, pe_id(PE::PE0), 1, 4 /* 16 */, 1);
        ASSERT_EQ(kernel::TCU::send(2, nullptr, 0, 0x1111, TCU::NO_REPLIES), Errors::RECV_MISALIGN);
    }
}

static void test_msg_send_empty() {
    ALIGNED(8) char buffer[2 * 64];
    uintptr_t buf1 = reinterpret_cast<uintptr_t>(&buffer);

    kernel::TCU::config_recv(1, buf1, 6 /* 64 */, 6 /* 64 */, 3);
    kernel::TCU::config_send(4, 0x5678, pe_id(PE::PE0), 1, 4 /* 16 */, 1);

    // send empty message
    ASSERT_EQ(kernel::TCU::send(4, nullptr, 0, 0x2222, TCU::NO_REPLIES), Errors::NONE);

    // fetch message
    const TCU::Message *rmsg;
    while((rmsg = kernel::TCU::fetch_msg(1, buf1)) == nullptr)
        ;
    // validate contents
    ASSERT_EQ(rmsg->label, 0x5678);
    ASSERT_EQ(rmsg->replylabel, 0x2222);
    ASSERT_EQ(rmsg->length, 0);
    ASSERT_EQ(rmsg->senderEp, 4);
    ASSERT_EQ(rmsg->replySize, 4 /* log2(TCU::Message::Header) */);
    ASSERT_EQ(rmsg->replyEp, TCU::INVALID_EP);
    ASSERT_EQ(rmsg->senderPe, pe_id(PE::PE0));
    ASSERT_EQ(rmsg->flags, 0);

    ASSERT_EQ(kernel::TCU::ack_msg(1, buf1, rmsg), Errors::NONE);
}

static void test_msg_reply_empty() {
    ALIGNED(8) char buffer[2 * 64];
    ALIGNED(8) char buffer2[2 * 64];
    uintptr_t buf1 = reinterpret_cast<uintptr_t>(&buffer);
    uintptr_t buf2 = reinterpret_cast<uintptr_t>(&buffer2);

    kernel::TCU::config_recv(1, buf1, 6 /* 64 */, 6 /* 64 */, 3);
    kernel::TCU::config_recv(2, buf2, 6 /* 64 */, 6 /* 64 */, TCU::NO_REPLIES);
    kernel::TCU::config_send(4, 0x1234, pe_id(PE::PE0), 1, 4 /* 16 */, 1);

    // send empty message
    ASSERT_EQ(kernel::TCU::credits(4), 1);
    ASSERT_EQ(kernel::TCU::send(4, nullptr, 0, 0x1111, 2), Errors::NONE);
    ASSERT_EQ(kernel::TCU::credits(4), 0);

    // fetch message
    const TCU::Message *rmsg;
    while((rmsg = kernel::TCU::fetch_msg(1, buf1)) == nullptr)
        ;
    // validate contents
    ASSERT_EQ(rmsg->label, 0x1234);
    ASSERT_EQ(rmsg->replylabel, 0x1111);
    ASSERT_EQ(rmsg->length, 0);
    ASSERT_EQ(rmsg->senderEp, 4);
    ASSERT_EQ(rmsg->replySize, 6);
    ASSERT_EQ(rmsg->replyEp, 2);
    ASSERT_EQ(rmsg->senderPe, pe_id(PE::PE0));
    ASSERT_EQ(rmsg->flags, 0);

    // sending with the use-once send EP is not allowed
    ASSERT_EQ(kernel::TCU::send(3, nullptr, 0, 0x1111, TCU::NO_REPLIES), Errors::SEND_REPLY_EP);
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
    ASSERT_EQ(rmsg->replyEp, 4);
    ASSERT_EQ(rmsg->senderPe, pe_id(PE::PE0));
    ASSERT_EQ(rmsg->flags, TCU::Header::FL_REPLY);
    // free slot
    ASSERT_EQ(kernel::TCU::ack_msg(2, buf2, rmsg), Errors::NONE);
}

static void test_msg_no_reply() {
    ALIGNED(8) char buffer[2 * 64];
    ALIGNED(8) char buffer2[2 * 64];
    uintptr_t buf1 = reinterpret_cast<uintptr_t>(&buffer);
    uintptr_t buf2 = reinterpret_cast<uintptr_t>(&buffer2);

    uint64_t msg = 5678;
    uint64_t reply = 9123;

    kernel::TCU::config_recv(1, buf1, 7 /* 128 */, 6 /* 64 */, 3);
    kernel::TCU::config_recv(2, buf2, 6 /* 64 */, 6 /* 64 */, TCU::NO_REPLIES);
    kernel::TCU::config_send(5, 0x1234, pe_id(PE::PE0), 1, 6 /* 64 */, 2);

    // send with replies disabled
    ASSERT_EQ(kernel::TCU::credits(5), 2);
    ASSERT_EQ(kernel::TCU::send(5, &msg, sizeof(msg), 0x1111, TCU::NO_REPLIES), Errors::NONE);
    ASSERT_EQ(kernel::TCU::credits(5), 1);

    // fetch message
    const TCU::Message *rmsg;
    while((rmsg = kernel::TCU::fetch_msg(1, buf1)) == nullptr)
        ;
    // validate contents
    ASSERT_EQ(rmsg->label, 0x1234);
    ASSERT_EQ(rmsg->replylabel, 0x1111);
    ASSERT_EQ(rmsg->length, sizeof(msg));
    ASSERT_EQ(rmsg->senderEp, 5);
    ASSERT_EQ(rmsg->replySize, 4 /* log2(TCU::Message::Header) */);
    ASSERT_EQ(rmsg->replyEp, TCU::INVALID_EP);
    ASSERT_EQ(rmsg->senderPe, pe_id(PE::PE0));
    ASSERT_EQ(rmsg->flags, 0);
    const uint64_t *msg_ctrl = reinterpret_cast<const uint64_t*>(rmsg->data);
    ASSERT_EQ(*msg_ctrl, msg);

    // reply with data not allowed
    ASSERT_EQ(kernel::TCU::reply(1, &reply, sizeof(reply), buf1, rmsg), Errors::NO_SEP);
    // empty reply is not allowed
    ASSERT_EQ(kernel::TCU::reply(1, nullptr, 0, buf1, rmsg), Errors::NO_SEP);
    ASSERT_EQ(kernel::TCU::ack_msg(1, buf1, rmsg), Errors::NONE);
}

static void test_msg_no_credits() {
    ALIGNED(8) char buffer[2 * 64];
    ALIGNED(8) char buffer2[2 * 64];
    uintptr_t buf1 = reinterpret_cast<uintptr_t>(&buffer);
    uintptr_t buf2 = reinterpret_cast<uintptr_t>(&buffer2);

    uint64_t msg = 5678;
    uint64_t reply = 9123;

    kernel::TCU::config_recv(1, buf1, 7 /* 128 */, 6 /* 64 */, 3);
    kernel::TCU::config_recv(2, buf2, 6 /* 64 */, 6 /* 64 */, TCU::NO_REPLIES);
    kernel::TCU::config_send(5, 0x1234, pe_id(PE::PE0), 1, 6 /* 64 */, TCU::UNLIM_CREDITS);

    // send without credits
    ASSERT_EQ(kernel::TCU::credits(5), TCU::UNLIM_CREDITS);
    ASSERT_EQ(kernel::TCU::send(5, &msg, sizeof(msg), 0x1111, 2), Errors::NONE);
    ASSERT_EQ(kernel::TCU::send(5, &msg, sizeof(msg), 0x1111, 2), Errors::NONE);
    // receive buffer full
    ASSERT_EQ(kernel::TCU::send(5, &msg, sizeof(msg), 0x1111, 2), Errors::RECV_NO_SPACE);
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
    ASSERT_EQ(rmsg->replySize, 6);
    ASSERT_EQ(rmsg->replyEp, 2);
    ASSERT_EQ(rmsg->senderPe, pe_id(PE::PE0));
    ASSERT_EQ(rmsg->flags, 0);
    const uint64_t *msg_ctrl = reinterpret_cast<const uint64_t*>(rmsg->data);
    ASSERT_EQ(*msg_ctrl, msg);

    // send empty reply
    ASSERT_EQ(kernel::TCU::reply(1, &reply, sizeof(reply), buf1, rmsg), Errors::NONE);

    // fetch reply
    while((rmsg = kernel::TCU::fetch_msg(2, buf2)) == nullptr)
        ;
    // validate contents
    ASSERT_EQ(rmsg->label, 0x1111);
    ASSERT_EQ(rmsg->length, 8);
    ASSERT_EQ(rmsg->senderEp, 1);
    ASSERT_EQ(rmsg->replySize, 0);
    ASSERT_EQ(rmsg->replyEp, TCU::INVALID_EP);
    ASSERT_EQ(rmsg->senderPe, pe_id(PE::PE0));
    ASSERT_EQ(rmsg->flags, TCU::Header::FL_REPLY);
    const uint64_t *reply_ctrl = reinterpret_cast<const uint64_t*>(rmsg->data);
    ASSERT_EQ(*reply_ctrl, reply);
    // free slot
    ASSERT_EQ(kernel::TCU::ack_msg(2, buf2, rmsg), Errors::NONE);

    // credits are still the same
    ASSERT_EQ(kernel::TCU::credits(5), TCU::UNLIM_CREDITS);

    // ack the other message we sent above
    rmsg = kernel::TCU::fetch_msg(1, buf1);
    ASSERT(rmsg != nullptr);
    ASSERT_EQ(kernel::TCU::ack_msg(1, buf1, rmsg), Errors::NONE);
}

static void test_msg_2send_2reply() {
    ALIGNED(8) char buffer[2 * 64];
    ALIGNED(8) char buffer2[2 * 64];
    uintptr_t buf1 = reinterpret_cast<uintptr_t>(&buffer);
    uintptr_t buf2 = reinterpret_cast<uintptr_t>(&buffer2);

    uint64_t msg = 5678;
    uint64_t reply = 9123;

    kernel::TCU::config_recv(1, buf1, 7 /* 128 */, 6 /* 64 */, 3);
    kernel::TCU::config_recv(2, buf2, 7 /* 128 */, 6 /* 64 */, TCU::NO_REPLIES);
    kernel::TCU::config_send(5, 0x1234, pe_id(PE::PE0), 1, 6 /* 64 */, 2);

    // send twice
    ASSERT_EQ(kernel::TCU::send(5, &msg, sizeof(msg), 0x1111, 2), Errors::NONE);
    ASSERT_EQ(kernel::TCU::send(5, &msg, sizeof(msg), 0x2222, 2), Errors::NONE);
    // we need the reply to get our credits back
    ASSERT_EQ(kernel::TCU::send(5, &msg, sizeof(msg), 0, 2), Errors::NO_CREDITS);

    for(int i = 0; i < 2; ++i) {
        // fetch message
        const TCU::Message *rmsg;
        while((rmsg = kernel::TCU::fetch_msg(1, buf1)) == nullptr)
            ;
        // validate contents
        ASSERT_EQ(rmsg->label, 0x1234);
        ASSERT_EQ(rmsg->replylabel, i == 0 ? 0x1111 : 0x2222);
        ASSERT_EQ(rmsg->length, sizeof(msg));
        ASSERT_EQ(rmsg->senderEp, 5);
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
        // can't reply again (SEP invalid)
        ASSERT_EQ(kernel::TCU::reply(1, &reply, sizeof(reply), buf1, rmsg), Errors::NO_SEP);
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
        ASSERT_EQ(rmsg->replyEp, 5);
        ASSERT_EQ(rmsg->senderPe, pe_id(PE::PE0));
        ASSERT_EQ(rmsg->flags, TCU::Header::FL_REPLY);
        const uint64_t *msg_ctrl = reinterpret_cast<const uint64_t*>(rmsg->data);
        ASSERT_EQ(*msg_ctrl, reply);
        // free slot
        ASSERT_EQ(kernel::TCU::ack_msg(2, buf2, rmsg), Errors::NONE);
    }

    // credits are back
    ASSERT_EQ(kernel::TCU::credits(5), 2);
}

template<typename DATA>
static void test_msg(size_t msg_size_in, size_t reply_size_in) {
    Serial::get() << "SEND+REPLY with " << msg_size_in << " " << sizeof(DATA) << "B words\n";

    const size_t TOTAL_MSG_SIZE = msg_size_in * sizeof(DATA) + sizeof(TCU::Header);
    const size_t TOTAL_REPLY_SIZE = reply_size_in * sizeof(DATA) + sizeof(TCU::Header);

    ALIGNED(8) char rbuffer[2 * TOTAL_MSG_SIZE];
    ALIGNED(8) char rbuffer2[2 * TOTAL_REPLY_SIZE];
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

    kernel::TCU::config_recv(1, buf1, slot_msgsize + 1, slot_msgsize, 3);
    kernel::TCU::config_recv(2, buf2, slot_replysize + 1, slot_replysize, TCU::NO_REPLIES);
    kernel::TCU::config_send(4, 0x1234, pe_id(PE::PE0), 1, slot_msgsize, 1);

    ASSERT_EQ(kernel::TCU::send(4, msg, msg_size_in * sizeof(DATA), 0x1111, 2), Errors::NONE);

    // fetch message
    const TCU::Message *rmsg;
    while((rmsg = kernel::TCU::fetch_msg(1, buf1)) == nullptr)
        ;
    // validate contents
    ASSERT_EQ(rmsg->label, 0x1234);
    ASSERT_EQ(rmsg->replylabel, 0x1111);
    ASSERT_EQ(rmsg->length, msg_size_in * sizeof(DATA));
    ASSERT_EQ(rmsg->senderEp, 4);
    ASSERT_EQ(rmsg->replyEp, 2);
    ASSERT_EQ(rmsg->senderPe, pe_id(PE::PE0));
    ASSERT_EQ(rmsg->flags, 0);
    const DATA *msg_ctrl = reinterpret_cast<const DATA*>(rmsg->data);
    for(size_t i = 0; i < msg_size_in; ++i)
        ASSERT_EQ(msg_ctrl[i], msg[i]);

    // we need the reply to get our credits back
    ASSERT_EQ(kernel::TCU::send(4, msg, msg_size_in * sizeof(DATA), 0, 2), Errors::NO_CREDITS);

    // send reply
    ASSERT_EQ(kernel::TCU::reply(1, reply, reply_size_in * sizeof(DATA), buf1, rmsg), Errors::NONE);

    // fetch reply
    while((rmsg = kernel::TCU::fetch_msg(2, buf2)) == nullptr)
        ;
    // validate contents
    ASSERT_EQ(rmsg->label, 0x1111);
    ASSERT_EQ(rmsg->length, reply_size_in * sizeof(DATA));
    ASSERT_EQ(rmsg->senderEp, 1);
    ASSERT_EQ(rmsg->replyEp, 4);
    ASSERT_EQ(rmsg->senderPe, pe_id(PE::PE0));
    ASSERT_EQ(rmsg->flags, TCU::Header::FL_REPLY);
    msg_ctrl = reinterpret_cast<const DATA*>(rmsg->data);
    for(size_t i = 0; i < reply_size_in; ++i)
        ASSERT_EQ(msg_ctrl[i], reply[i]);
    // free slot
    ASSERT_EQ(kernel::TCU::ack_msg(2, buf2, rmsg), Errors::NONE);
}

template<size_t PAD>
struct UnalignedData {
    uint8_t _pad[PAD];
    uint64_t pre;
    uint64_t data[3];
    uint64_t post;
} PACKED ALIGNED(16);

template<size_t PAD>
static void test_unaligned_msg(size_t nwords) {
    Serial::get() << "SEND with " << PAD << "B padding and " << nwords << " words payload\n";

    const size_t TOTAL_MSG_ORD = m3::nextlog2<sizeof(UnalignedData<PAD>)>::val;

    ALIGNED(8) char rbuffer[1 << TOTAL_MSG_ORD];
    memset(rbuffer, -1, sizeof(rbuffer));
    uintptr_t buf = reinterpret_cast<uintptr_t>(&rbuffer);

    // prepare test data
    UnalignedData<PAD> msg;
    for(size_t i = 0; i < nwords; ++i)
        msg.data[i] = i + 1;

    kernel::TCU::config_recv(1, buf, TOTAL_MSG_ORD, TOTAL_MSG_ORD, TCU::NO_REPLIES);
    kernel::TCU::config_send(2, 0x1234, pe_id(PE::PE0), 1, TOTAL_MSG_ORD, 1);

    ASSERT_EQ(kernel::TCU::send(2, msg.data, nwords * sizeof(uint64_t), 0x5678, TCU::NO_REPLIES),
              Errors::NONE);

    // fetch message
    const TCU::Message *rmsg;
    while((rmsg = kernel::TCU::fetch_msg(1, buf)) == nullptr)
        ;

    // validate contents
    ASSERT_EQ(rmsg->label, 0x1234);
    ASSERT_EQ(rmsg->replylabel, 0x5678);
    ASSERT_EQ(rmsg->length, nwords * sizeof(uint64_t));
    ASSERT_EQ(rmsg->senderEp, 2);
    ASSERT_EQ(rmsg->replyEp, TCU::INVALID_EP);
    ASSERT_EQ(rmsg->senderPe, pe_id(PE::PE0));
    ASSERT_EQ(rmsg->flags, 0);
    const uint64_t *msg_ctrl = reinterpret_cast<const uint64_t*>(rmsg->data);
    for(size_t i = 0; i < nwords; ++i)
        ASSERT_EQ(msg_ctrl[i], i + 1);
    ASSERT_EQ(msg_ctrl[nwords], static_cast<uint64_t>(-1));

    // free slot
    ASSERT_EQ(kernel::TCU::ack_msg(1, buf, rmsg), Errors::NONE);
}

template<size_t PAD>
static void test_unaligned_rdwr(size_t nwords, size_t offset) {
    Serial::get() << "READ+WRITE with " << PAD << "B padding and "
                  << nwords << " words data from offset " << offset << "\n";

    // prepare test data
    UnalignedData<PAD> msg;
    msg.pre = 0xDEADBEEF;
    msg.post = 0xCAFEBABE;
    for(size_t i = 0; i < nwords; ++i)
        msg.data[i] = i + 1;

    kernel::TCU::config_mem(1, pe_id(PE::MEM), 0x1000, 0x1000, TCU::R | TCU::W);

    ASSERT_EQ(kernel::TCU::write(1, msg.data, nwords * sizeof(uint64_t), offset), Errors::NONE);
    ASSERT_EQ(kernel::TCU::read(1, msg.data, nwords * sizeof(uint64_t), offset), Errors::NONE);

    ASSERT_EQ(msg.pre, 0xDEADBEEF);
    ASSERT_EQ(msg.post, 0xCAFEBABE);
    for(size_t i = 0; i < nwords; ++i)
        ASSERT_EQ(msg.data[i], i + 1);
}

static void test_inv_ep() {
    ALIGNED(8) char rbuffer[32];
    uintptr_t buf = reinterpret_cast<uintptr_t>(&rbuffer);

    // force invalidate
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
        ASSERT_EQ(kernel::TCU::send(3, &data, sizeof(data), 0x5678, TCU::NO_REPLIES), Errors::NO_SEP);

        // invalidating again should work as well
        ASSERT_EQ(kernel::TCU::invalidate_ep_remote(pe_id(PE::PE0), 1, true), Errors::NONE);
        ASSERT_EQ(kernel::TCU::invalidate_ep_remote(pe_id(PE::PE0), 2, true), Errors::NONE);
        ASSERT_EQ(kernel::TCU::invalidate_ep_remote(pe_id(PE::PE0), 3, true), Errors::NONE);
    }

    // non-force send EP
    {
        kernel::TCU::config_recv(2, buf, 5 /* 32 */, 5 /* 32 */, TCU::INVALID_EP, 0, 0);
        kernel::TCU::config_send(3, 0x5678, pe_id(PE::PE0), 2, 5 /* 32 */, 1);

        // if credits are missing, we can't invalidate it (with force=0)
        uint64_t data;
        ASSERT_EQ(kernel::TCU::send(3, &data, sizeof(data), 0x5678, TCU::NO_REPLIES), Errors::NONE);
        ASSERT_EQ(kernel::TCU::invalidate_ep_remote(pe_id(PE::PE0), 3, false), Errors::NO_CREDITS);
        ASSERT_EQ(kernel::TCU::send(3, &data, sizeof(data), 0x5678, TCU::NO_REPLIES), Errors::NO_CREDITS);

        // with all credits, we can invalidate
        kernel::TCU::config_send(3, 0x5678, pe_id(PE::PE0), 2, 5 /* 32 */, 1);
        ASSERT_EQ(kernel::TCU::invalidate_ep_remote(pe_id(PE::PE0), 3, false), Errors::NONE);
        ASSERT_EQ(kernel::TCU::send(3, &data, sizeof(data), 0x5678, TCU::NO_REPLIES), Errors::NO_SEP);
    }

    // non-force receive EP
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

static void test_msg_receive() {
    ALIGNED(8) char rbuffer[32 * 32];
    uintptr_t buf = reinterpret_cast<uintptr_t>(&rbuffer);

    kernel::TCU::config_recv(2, buf, 5 + 5 /* 32 * 32 */, 5 /* 32 */, TCU::NO_REPLIES, 0, 0);
    kernel::TCU::config_send(3, 0x5678, pe_id(PE::PE0), 2, 5 /* 32 */, TCU::UNLIM_CREDITS);

    uint8_t expected_rpos = 0, expected_wpos = 0;
    for(int j = 0; j < 32; ++j) {
        // send all messages
        const uint64_t data = 0xDEADBEEF;
        for(int i = 0; i < j; ++i) {
            uint8_t rpos, wpos;
            kernel::TCU::recv_pos(2, &rpos, &wpos);
            ASSERT_EQ(rpos, expected_rpos);
            ASSERT_EQ(wpos, expected_wpos);

            ASSERT_EQ(kernel::TCU::send(3, &data, sizeof(data), static_cast<label_t>(i + 1),
                                        TCU::NO_REPLIES), Errors::NONE);
            if(expected_wpos == 32)
                expected_wpos = 1;
            else
                expected_wpos++;
        }

        // fetch all messages
        for(int i = 0; i < j; ++i) {
            uint8_t rpos, wpos;
            kernel::TCU::recv_pos(2, &rpos, &wpos);
            ASSERT_EQ(rpos, expected_rpos);
            ASSERT_EQ(wpos, expected_wpos);

            const TCU::Message *rmsg = kernel::TCU::fetch_msg(2, buf);
            ASSERT(rmsg != nullptr);

            if(expected_rpos == 32)
                expected_rpos = 1;
            else
                expected_rpos++;

            // validate contents
            ASSERT_EQ(rmsg->label, 0x5678);
            ASSERT_EQ(rmsg->replylabel, i + 1);

            // free slot
            ASSERT_EQ(kernel::TCU::ack_msg(2, buf, rmsg), Errors::NONE);
        }
    }
}

int main() {
    Serial::get() << "Starting TCU tests\n";

    test_msg_receive();
    test_mem_short();
    test_mem_large(PE::MEM);
    test_mem_large(PE::PE0);
    test_mem_rdwr(PE::MEM);
    test_msg_errors();
    test_msg_send_empty();
    test_msg_reply_empty();
    test_msg_no_reply();
    test_msg_no_credits();
    test_msg_2send_2reply();
    test_inv_ep();
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

    // test different alignments
    for(size_t i = 1; i <= 3; ++i) {
        test_unaligned_msg<1>(i);
        test_unaligned_msg<2>(i);
        test_unaligned_msg<3>(i);
        test_unaligned_msg<4>(i);
        test_unaligned_msg<5>(i);
        test_unaligned_msg<6>(i);
        test_unaligned_msg<7>(i);
        test_unaligned_msg<8>(i);
        test_unaligned_msg<9>(i);
        test_unaligned_msg<10>(i);
        test_unaligned_msg<11>(i);
        test_unaligned_msg<12>(i);
        test_unaligned_msg<13>(i);
        test_unaligned_msg<14>(i);
        test_unaligned_msg<15>(i);

        for(size_t off = 0; off < 16; off += 8) {
            test_unaligned_rdwr<1>(i, off);
            test_unaligned_rdwr<2>(i, off);
            test_unaligned_rdwr<3>(i, off);
            test_unaligned_rdwr<4>(i, off);
            test_unaligned_rdwr<5>(i, off);
            test_unaligned_rdwr<6>(i, off);
            test_unaligned_rdwr<7>(i, off);
            test_unaligned_rdwr<8>(i, off);
            test_unaligned_rdwr<9>(i, off);
            test_unaligned_rdwr<10>(i, off);
            test_unaligned_rdwr<11>(i, off);
            test_unaligned_rdwr<12>(i, off);
            test_unaligned_rdwr<13>(i, off);
            test_unaligned_rdwr<14>(i, off);
            test_unaligned_rdwr<15>(i, off);
            test_unaligned_rdwr<16>(i, off);
        }
    }

    Serial::get() << "\x1B[1;32mAll tests successful!\x1B[0;m\n";
    // for the test infrastructure
    Serial::get() << "Shutting down\n";
    return 0;
}
