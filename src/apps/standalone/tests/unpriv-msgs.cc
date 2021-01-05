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

static void test_msg_errors() {
    ALIGNED(8) char buffer[2 * 64];
    uintptr_t buf1 = reinterpret_cast<uintptr_t>(&buffer);

    uint64_t msg = 5678;

    Serial::get() << "SEND without send EP\n";
    {
        kernel::TCU::config_recv(1, buf1, 6 /* 64 */, 6 /* 64 */, 3);
        ASSERT_EQ(kernel::TCU::send(1, &msg, sizeof(msg), 0x1111, TCU::NO_REPLIES), Errors::NO_SEP);
    }

    Serial::get() << "SEND+ACK with invalid arguments\n";
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

    Serial::get() << "REPLY+ACK with out-of-bounds message\n";
    {
        kernel::TCU::config_recv(1, buf1, 6 /* 64 */, 6 /* 64 */, 3);

        // reply on message that's out of bounds
        const TCU::Message *rmsg = reinterpret_cast<const TCU::Message*>(buf1 + (1 << 6));
        ASSERT_EQ(kernel::TCU::reply(1, nullptr, 0, buf1, rmsg), Errors::INV_MSG_OFF);
        // ack message that's out of bounds
        ASSERT_EQ(kernel::TCU::ack_msg(1, buf1, rmsg), Errors::INV_MSG_OFF);
    }

    Serial::get() << "REPLY with disabled replies\n";
    {
        kernel::TCU::config_recv(1, buf1, 6 /* 64 */, 6 /* 64 */, TCU::NO_REPLIES);
        const TCU::Message *rmsg = reinterpret_cast<const TCU::Message*>(buf1);
        ASSERT_EQ(kernel::TCU::reply(1, nullptr, 0, buf1, rmsg), Errors::REPLIES_DISABLED);
    }

    Serial::get() << "REPLY with normal send EP\n";
    {
        kernel::TCU::config_recv(1, buf1, 6 /* 64 */, 6 /* 64 */, 2,
                                 1 /* make msg 0 (EP 2) occupied */, 0);
        kernel::TCU::config_send(2, 0x5678, pe_id(PE::PE0), 1, 4 /* 16 */, 1);
        const TCU::Message *rmsg = reinterpret_cast<const TCU::Message*>(buf1);
        ASSERT_EQ(kernel::TCU::reply(1, nullptr, 0, buf1, rmsg), Errors::SEND_REPLY_EP);
    }

    Serial::get() << "SEND to invalid receive EP\n";
    {
        kernel::TCU::config_invalid(2);
        kernel::TCU::config_send(1, 0x5678, pe_id(PE::PE0), 2 /* invalid REP */, 4 /* 16 */, 1);
        ASSERT_EQ(kernel::TCU::send(1, nullptr, 0, 0x1111, TCU::NO_REPLIES), Errors::RECV_GONE);
    }

    Serial::get() << "SEND to out-of-bounds receive EP\n";
    {
        kernel::TCU::config_send(1, 0x5678, pe_id(PE::PE0), TOTAL_EPS, 4 /* 16 */, 1);
        ASSERT_EQ(kernel::TCU::send(1, nullptr, 0, 0x1111, TCU::NO_REPLIES), Errors::RECV_GONE);
    }

    Serial::get() << "SEND to receive EP with misaligned receive buffer\n";
    {
        kernel::TCU::config_recv(1, buf1 + 1 /* misaligned */, 6 /* 64 */, 6 /* 64 */, TCU::NO_REPLIES);
        kernel::TCU::config_send(2, 0x5678, pe_id(PE::PE0), 1, 4 /* 16 */, 1);
        ASSERT_EQ(kernel::TCU::send(2, nullptr, 0, 0x1111, TCU::NO_REPLIES), Errors::RECV_MISALIGN);
    }

    Serial::get() << "SEND of too large message\n";
    {
        kernel::TCU::config_recv(1, buf1, 5 /* 32 */, 5 /* 32 */, TCU::NO_REPLIES);
        kernel::TCU::config_send(2, 0x5678, pe_id(PE::PE0), 1, 6 /* 64 */, 1);
        uint64_t data[6];
        ASSERT_EQ(kernel::TCU::send(2, &data, sizeof(data), 0x1111, TCU::NO_REPLIES), Errors::RECV_OUT_OF_BOUNDS);
    }

    Serial::get() << "SEND+ACK+REPLY with invalid reply EPs\n";
    {
        kernel::TCU::config_recv(1, buf1, 5 /* 32 */, 5 /* 32 */, TOTAL_EPS);
        kernel::TCU::config_send(2, 0x5678, pe_id(PE::PE0), 1, 5 /* 32 */, 1);
        ASSERT_EQ(kernel::TCU::send(2, nullptr, 0, 0x1111, 1), Errors::RECV_INV_RPL_EPS);
        auto rmsg = reinterpret_cast<const m3::TCU::Message*>(buffer);
        ASSERT_EQ(kernel::TCU::ack_msg(1, buf1, rmsg), Errors::RECV_INV_RPL_EPS);
        ASSERT_EQ(kernel::TCU::reply(1, nullptr, 0, buf1, rmsg), Errors::RECV_INV_RPL_EPS);
    }

    Serial::get() << "SEND+REPLY with invalid credit EP\n";
    {
        kernel::TCU::config_recv(1, buf1, 5 /* 32 */, 5 /* 32 */, 2);
        // install reply EP
        kernel::TCU::config_send(2, 0x5678, pe_id(PE::PE0), 1, 5 /* 32 */, 1, true, TOTAL_EPS);
        // now try to reply with invalid credit EP
        auto rmsg = reinterpret_cast<const m3::TCU::Message*>(buffer);
        ASSERT_EQ(kernel::TCU::reply(1, nullptr, 0, buf1, rmsg), Errors::SEND_INV_CRD_EP);
    }

    Serial::get() << "SEND with invalid message size\n";
    {
        kernel::TCU::config_send(2, 0x5678, pe_id(PE::PE0), 1, 12 /* 4096 */, 1);
        uint64_t data[6];
        ASSERT_EQ(kernel::TCU::send(2, &data, sizeof(data), 0x1111, TCU::NO_REPLIES), Errors::SEND_INV_MSG_SZ);
    }

    Serial::get() << "REPLY with invalid message size in reply EP\n";
    {
        kernel::TCU::config_recv(1, buf1, 5 /* 32 */, 5 /* 32 */, 3);
        kernel::TCU::config_send(2, 0x5678, pe_id(PE::PE0), 1, 5 /* 32 */, 1);
        // install reply EP
        kernel::TCU::config_send(3, 0x5678, pe_id(PE::PE0), 1, 12 /* 4096 */, 1, true, 2);
        // now try to reply
        auto rmsg = reinterpret_cast<const m3::TCU::Message*>(buffer);
        ASSERT_EQ(kernel::TCU::reply(1, nullptr, 0, buf1, rmsg), Errors::SEND_INV_MSG_SZ);
    }

    Serial::get() << "Send EP should not lose credits on failed SENDs\n";
    {
        kernel::TCU::config_invalid(1);
        kernel::TCU::config_send(2, 0x5678, pe_id(PE::PE0), 1, 5 /* 32 */, 1);
        // try send to invalid receive EP
        ASSERT_EQ(kernel::TCU::send(2, nullptr, 0, 0x1111, TCU::NO_REPLIES), Errors::RECV_GONE);
        // now we should still have credits
        ASSERT_EQ(kernel::TCU::credits(2), 1);
    }

    Serial::get() << "Receive EP should not change on failed REPLYs\n";
    {
        kernel::TCU::config_recv(1, buf1, 5 /* 32 */, 5 /* 32 */, 3, 0x1, 0x1);
        kernel::TCU::config_send(2, 0x5678, pe_id(PE::PE0), 1, 5 /* 32 */, 1);
        // install reply EP
        kernel::TCU::config_send(3, 0x5678, pe_id(PE::PE0), 4, 5 /* 32 */, 1, true, 2);
        kernel::TCU::config_invalid(4);
        // now try reply to invalid receive EP
        auto rmsg = reinterpret_cast<const m3::TCU::Message*>(buffer);
        ASSERT_EQ(kernel::TCU::reply(1, nullptr, 0, buf1, rmsg), Errors::RECV_GONE);

        // now we should still have credits and the msg should still be unread
        ASSERT_EQ(kernel::TCU::credits(3), 1);
        uint32_t unread, occupied;
        kernel::TCU::recv_masks(1, &unread, &occupied);
        ASSERT_EQ(unread, 0x1);
        ASSERT_EQ(occupied, 0x1);
    }
}

static void test_msg_send_empty() {
    Serial::get() << "SEND with empty message\n";

    ALIGNED(8) char buffer[2 * 64];
    uintptr_t buf1 = reinterpret_cast<uintptr_t>(&buffer);

    kernel::TCU::config_recv(1, buf1, 6 /* 64 */, 6 /* 64 */, 3);
    kernel::TCU::config_send(4, 0x5678, pe_id(PE::PE0), 1, 4 /* 16 */, 1);

    // send empty message
    ASSERT_EQ(kernel::TCU::send(4, nullptr, 0, 0x2222, TCU::NO_REPLIES), Errors::NONE);
    ASSERT_EQ(kernel::TCU::max_credits(4), 1);
    ASSERT_EQ(kernel::TCU::credits(4), 0);

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
    Serial::get() << "REPLY with empty message\n";

    ALIGNED(8) char buffer[2 * 64];
    ALIGNED(8) char buffer2[2 * 64];
    uintptr_t buf1 = reinterpret_cast<uintptr_t>(&buffer);
    uintptr_t buf2 = reinterpret_cast<uintptr_t>(&buffer2);

    kernel::TCU::config_recv(1, buf1, 6 /* 64 */, 6 /* 64 */, 3);
    kernel::TCU::config_recv(2, buf2, 6 /* 64 */, 6 /* 64 */, TCU::NO_REPLIES);
    kernel::TCU::config_send(4, 0x1234, pe_id(PE::PE0), 1, 4 /* 16 */, 1);

    // send empty message
    ASSERT_EQ(kernel::TCU::max_credits(4), 1);
    ASSERT_EQ(kernel::TCU::credits(4), 1);
    ASSERT_EQ(kernel::TCU::send(4, nullptr, 0, 0x1111, 2), Errors::NONE);
    ASSERT_EQ(kernel::TCU::max_credits(4), 1);
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

    ASSERT_EQ(kernel::TCU::max_credits(4), 1);
    ASSERT_EQ(kernel::TCU::credits(4), 1);

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
    Serial::get() << "SEND without reply\n";

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
    Serial::get() << "SEND without credits\n";

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
    Serial::get() << "Two SENDs and two REPLYs\n";

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

static void test_msg_receive() {
    Serial::get() << "SEND+FETCH and verify unread/occupied/rpos/wpos\n";

    ALIGNED(8) char rbuffer[32 * 32];
    uintptr_t buf = reinterpret_cast<uintptr_t>(&rbuffer);

    kernel::TCU::config_recv(2, buf, 5 + 5 /* 32 * 32 */, 5 /* 32 */, TCU::NO_REPLIES, 0, 0);
    kernel::TCU::config_send(3, 0x5678, pe_id(PE::PE0), 2, 5 /* 32 */, TCU::UNLIM_CREDITS);

    uint8_t expected_rpos = 0, expected_wpos = 0;
    uint32_t expected_unread = 0, expected_occupied = 0;
    for(int j = 0; j < 32; ++j) {
        // send all messages
        const uint64_t data = 0xDEADBEEF;
        for(int i = 0; i < j; ++i) {
            uint8_t rpos, wpos;
            uint32_t unread, occupied;
            kernel::TCU::recv_pos(2, &rpos, &wpos);
            kernel::TCU::recv_masks(2, &unread, &occupied);
            ASSERT_EQ(rpos, expected_rpos);
            ASSERT_EQ(wpos, expected_wpos);
            ASSERT_EQ(unread, expected_unread);
            ASSERT_EQ(occupied, expected_occupied);

            ASSERT_EQ(kernel::TCU::send(3, &data, sizeof(data), static_cast<label_t>(i + 1),
                                        TCU::NO_REPLIES), Errors::NONE);
            if(wpos == 32) {
                expected_unread |= 1 << 0;
                expected_occupied |= 1 << 0;
            }
            else {
                expected_unread |= 1 << wpos;
                expected_occupied |= 1 << wpos;
            }

            if(expected_wpos == 32)
                expected_wpos = 1;
            else
                expected_wpos++;
        }

        // fetch all messages
        for(int i = 0; i < j; ++i) {
            uint8_t rpos, wpos;
            uint32_t unread, occupied;
            kernel::TCU::recv_pos(2, &rpos, &wpos);
            kernel::TCU::recv_masks(2, &unread, &occupied);
            ASSERT_EQ(rpos, expected_rpos);
            ASSERT_EQ(wpos, expected_wpos);
            ASSERT_EQ(unread, expected_unread);
            ASSERT_EQ(occupied, expected_occupied);

            const TCU::Message *rmsg = kernel::TCU::fetch_msg(2, buf);
            ASSERT(rmsg != nullptr);

            if(rpos == 32)
                expected_unread &= ~(1U << 0);
            else
                expected_unread &= ~(1U << rpos);

            if(expected_rpos == 32)
                expected_rpos = 1;
            else
                expected_rpos++;

            kernel::TCU::recv_masks(2, &unread, &occupied);
            ASSERT_EQ(unread, expected_unread);
            ASSERT_EQ(occupied, expected_occupied);

            // validate contents
            ASSERT_EQ(rmsg->label, 0x5678);
            ASSERT_EQ(rmsg->replylabel, static_cast<uint32_t>(i + 1));

            // free slot
            ASSERT_EQ(kernel::TCU::ack_msg(2, buf, rmsg), Errors::NONE);

            if(rpos == 32)
                expected_occupied &= ~(1U << 0);
            else
                expected_occupied &= ~(1U << rpos);
        }
    }
}

void test_msgs() {
    test_msg_receive();
    test_msg_errors();
    test_msg_send_empty();
    test_msg_reply_empty();
    test_msg_no_reply();
    test_msg_no_credits();
    test_msg_2send_2reply();

    // test different lengths
    for(size_t i = 1; i <= 80; i++) {
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
    }
}
