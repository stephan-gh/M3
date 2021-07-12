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

static constexpr epid_t MEP = TCU::FIRST_USER_EP;
static constexpr epid_t MEP2 = TCU::FIRST_USER_EP + 1;
static constexpr epid_t SEP = TCU::FIRST_USER_EP + 2;
static constexpr epid_t REP = TCU::FIRST_USER_EP + 3;

static uint8_t src_buf[16384];
static uint8_t dst_buf[16384];
static uint8_t mem_buf[16384];

static void test_mem_short() {
    uint64_t data = 1234;

    ASSERT_EQ(kernel::TCU::unknown_cmd(), Errors::UNKNOWN_CMD);

    kernel::TCU::config_mem(MEP, pe_id(PE::MEM), 0x1000, sizeof(uint64_t), TCU::R | TCU::W);

    Serial::get() << "WRITE with invalid arguments\n";
    {
        kernel::TCU::config_mem(MEP2, pe_id(PE::MEM), 0x1000, sizeof(uint64_t), TCU::R);
        kernel::TCU::config_send(SEP, 0x1234, pe_id(PE::PE0), REP, 6 /* 64 */, 2);

        // not a memory EP
        ASSERT_EQ(kernel::TCU::write(SEP, &data, sizeof(data), 0), Errors::NO_MEP);
        // offset out of bounds
        ASSERT_EQ(kernel::TCU::write(MEP, &data, sizeof(data), 1), Errors::OUT_OF_BOUNDS);
        // size out of bounds
        ASSERT_EQ(kernel::TCU::write(MEP, &data, sizeof(data) + 1, 0), Errors::OUT_OF_BOUNDS);
        // no write permission
        ASSERT_EQ(kernel::TCU::write(MEP2, &data, sizeof(data), 0), Errors::NO_PERM);
    }

    Serial::get() << "READ with invalid arguments\n";
    {
        kernel::TCU::config_mem(MEP2, pe_id(PE::MEM), 0x1000, sizeof(uint64_t), TCU::W);
        kernel::TCU::config_send(SEP, 0x1234, pe_id(PE::PE0), REP, 6 /* 64 */, 2);

        // not a memory EP
        ASSERT_EQ(kernel::TCU::read(SEP, &data, sizeof(data), 0), Errors::NO_MEP);
        // offset out of bounds
        ASSERT_EQ(kernel::TCU::read(MEP, &data, sizeof(data), 1), Errors::OUT_OF_BOUNDS);
        // size out of bounds
        ASSERT_EQ(kernel::TCU::read(MEP, &data, sizeof(data) + 1, 0), Errors::OUT_OF_BOUNDS);
        // no read permission
        ASSERT_EQ(kernel::TCU::read(MEP2, &data, sizeof(data), 0), Errors::NO_PERM);
    }

    Serial::get() << "READ+WRITE with offset = 0\n";
    {
        uint64_t data_ctrl = 0;
        ASSERT_EQ(kernel::TCU::write(MEP, &data, sizeof(data), 0), Errors::NONE);
        ASSERT_EQ(kernel::TCU::read(MEP, &data_ctrl, sizeof(data), 0), Errors::NONE);
        ASSERT_EQ(data, data_ctrl);
    }

    Serial::get() << "READ+WRITE with offset != 0\n";
    {
        kernel::TCU::config_mem(MEP2, pe_id(PE::MEM), 0x2000, sizeof(uint64_t) * 2, TCU::R| TCU::W);

        uint64_t data_ctrl = 0;
        ASSERT_EQ(kernel::TCU::write(MEP2, &data, sizeof(data), 4), Errors::NONE);
        ASSERT_EQ(kernel::TCU::read(MEP2, &data_ctrl, sizeof(data), 4), Errors::NONE);
        ASSERT_EQ(data, data_ctrl);
    }

    Serial::get() << "0-byte READ+WRITE transfers\n";
    {
        kernel::TCU::config_mem(MEP2, pe_id(PE::MEM), 0x2000, sizeof(uint64_t) * 2, TCU::R| TCU::W);

        ASSERT_EQ(kernel::TCU::write(MEP2, nullptr, 0, 0), Errors::NONE);
        ASSERT_EQ(kernel::TCU::read(MEP2, nullptr, 0, 0), Errors::NONE);
    }
}

static void test_mem_large(PE mem_pe) {
    for(size_t i = 0; i < ARRAY_SIZE(src_buf); ++i)
        src_buf[i] = i;

    size_t addr = mem_pe == PE::MEM ? 0x1000 : reinterpret_cast<size_t>(mem_buf);
    kernel::TCU::config_mem(MEP, pe_id(mem_pe), addr, sizeof(src_buf), TCU::R | TCU::W);

    const size_t sizes[] = {64, 128, 256, 512, 1024, 2048, 4096, 8192, 16384};
    for(auto size : sizes) {
        Serial::get() << "READ+WRITE with " << size << " bytes with PE" << (int)mem_pe << "\n";

        ASSERT_EQ(kernel::TCU::write(MEP, src_buf, size, 0), Errors::NONE);
        ASSERT_EQ(kernel::TCU::read(MEP, dst_buf, size, 0), Errors::NONE);
        for(size_t i = 0; i < size; ++i)
            ASSERT_EQ(src_buf[i], dst_buf[i]);
    }
}

static void test_mem_rdwr(PE mem_pe) {
    for(size_t i = 0; i < ARRAY_SIZE(src_buf); ++i)
        src_buf[i] = i;

    size_t addr = mem_pe == PE::MEM ? 0x1000 : reinterpret_cast<size_t>(mem_buf);
    kernel::TCU::config_mem(MEP, pe_id(mem_pe), addr, sizeof(src_buf), TCU::R | TCU::W);

    const size_t sizes[] = {4096, 8192};
    for(auto size : sizes) {
        memset(dst_buf, 0, sizeof(dst_buf));

        Serial::get() << "READ+WRITE+READ+WRITE with " << size << " bytes with PE" << (int)mem_pe << "\n";

        // first write our data
        ASSERT_EQ(kernel::TCU::write(MEP, src_buf, size, 0), Errors::NONE);
        // read it into a buffer for the next write
        ASSERT_EQ(kernel::TCU::read(MEP, dst_buf, size, 0), Errors::NONE);
        // write the just read data
        ASSERT_EQ(kernel::TCU::write(MEP, dst_buf, size, 0), Errors::NONE);
        // read it again for checking purposes
        ASSERT_EQ(kernel::TCU::read(MEP, dst_buf, size, 0), Errors::NONE);
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

    kernel::TCU::config_mem(MEP, pe_id(PE::MEM), 0x1000, size_in * sizeof(DATA), TCU::R | TCU::W);

    // test write + read
    ASSERT_EQ(kernel::TCU::write(MEP, msg, size_in * sizeof(DATA), 0), Errors::NONE);
    ASSERT_EQ(kernel::TCU::read(MEP, buffer, size_in * sizeof(DATA), 0), Errors::NONE);
    for(size_t i = 0; i < size_in; i++)
        ASSERT_EQ(buffer[i], msg[i]);
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

    kernel::TCU::config_mem(MEP, pe_id(PE::MEM), 0x1000, 0x1000, TCU::R | TCU::W);

    ASSERT_EQ(kernel::TCU::write(MEP, msg.data, nwords * sizeof(uint64_t), offset), Errors::NONE);
    ASSERT_EQ(kernel::TCU::read(MEP, msg.data, nwords * sizeof(uint64_t), offset), Errors::NONE);

    ASSERT_EQ(msg.pre, 0xDEADBEEF);
    ASSERT_EQ(msg.post, 0xCAFEBABE);
    for(size_t i = 0; i < nwords; ++i)
        ASSERT_EQ(msg.data[i], i + 1);
}

void test_mem() {
    test_mem_short();
    test_mem_large(PE::MEM);
    test_mem_large(PE::PE0);
    test_mem_rdwr(PE::MEM);

    // test different lengths
    for(size_t i = 1; i <= 80; i++) {
        test_mem<uint8_t>(i);
        test_mem<uint16_t>(i);
        test_mem<uint32_t>(i);
        test_mem<uint64_t>(i);
    }

    // test different alignments
    for(size_t i = 1; i <= 3; ++i) {
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
}
