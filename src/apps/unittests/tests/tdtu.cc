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

#if defined(__host__)
#include <base/Env.h>

#include <m3/com/MemGate.h>
#include <m3/com/RecvGate.h>
#include <m3/com/GateStream.h>
#include <m3/stream/Standard.h>
#include <m3/Test.h>

#include <sys/mman.h>

#include "../unittests.h"

using namespace m3;

static void *map_page() {
    void *addr = mmap(0, 0x1000, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANON, -1, 0);
    if(addr == MAP_FAILED) {
        exitmsg("mmap failed. Skipping test.");
        return nullptr;
    }
    return addr;
}
static void unmap_page(void *addr) {
    munmap(addr, 0x1000);
}

static void dmacmd(const void *data, size_t len, epid_t ep, size_t offset, size_t length, int op) {
    m3::DTU &dtu = m3::DTU::get();
    dtu.set_cmd(m3::DTU::CMD_ADDR, reinterpret_cast<word_t>(data));
    dtu.set_cmd(m3::DTU::CMD_SIZE, len);
    dtu.set_cmd(m3::DTU::CMD_EPID, ep);
    dtu.set_cmd(m3::DTU::CMD_OFFSET, offset);
    dtu.set_cmd(m3::DTU::CMD_LENGTH, length);
    dtu.set_cmd(m3::DTU::CMD_REPLYLBL, 0);
    dtu.set_cmd(m3::DTU::CMD_REPLY_EPID, 0);
    dtu.set_cmd(m3::DTU::CMD_CTRL, static_cast<word_t>(op << 3) | m3::DTU::CTRL_START |
                                   m3::DTU::CTRL_DEL_REPLY_CAP);
    dtu.exec_command();
}

static void cmds_read() {
    const epid_t rcvep = VPE::self().alloc_ep();
    const epid_t sndep = VPE::self().alloc_ep();
    DTU &dtu = DTU::get();

    void *addr = map_page();
    if(!addr)
        return;

    const size_t datasize = sizeof(word_t) * 4;
    word_t *data = reinterpret_cast<word_t*>(addr);
    data[0] = 1234;
    data[1] = 5678;
    data[2] = 1122;
    data[3] = 3344;

    cout << "-- Test errors --\n";
    {
        dtu.configure(sndep, reinterpret_cast<word_t>(data) | MemGate::R, env()->pe,
            rcvep, datasize, 0);

        dmacmd(nullptr, 0, sndep, 0, datasize, DTU::WRITE);
        WVASSERTEQ(dtu.get_cmd(DTU::CMD_ERROR), static_cast<word_t>(Errors::NO_PERM));

        dmacmd(nullptr, 0, sndep, 0, datasize + 1, DTU::READ);
        WVASSERTEQ(dtu.get_cmd(DTU::CMD_ERROR), static_cast<word_t>(Errors::INV_ARGS));

        dmacmd(nullptr, 0, sndep, datasize, 0, DTU::READ);
        WVASSERTEQ(dtu.get_cmd(DTU::CMD_ERROR), static_cast<word_t>(Errors::INV_ARGS));

        dmacmd(nullptr, 0, sndep, sizeof(word_t), datasize, DTU::READ);
        WVASSERTEQ(dtu.get_cmd(DTU::CMD_ERROR), static_cast<word_t>(Errors::INV_ARGS));
    }

    cout << "-- Test reading --\n";
    {
        dtu.configure(sndep, reinterpret_cast<word_t>(data) | MemGate::R, env()->pe,
            rcvep, datasize, 0);

        word_t buf[datasize / sizeof(word_t)];

        dmacmd(buf, datasize, sndep, 0, datasize, DTU::READ);
        WVASSERTEQ(dtu.get_cmd(DTU::CMD_ERROR), static_cast<word_t>(Errors::NONE));
        for(size_t i = 0; i < 4; ++i)
            WVASSERTEQ(buf[i], data[i]);
    }

    unmap_page(addr);
    dtu.configure(sndep, 0, 0, 0, 0, 0);
    VPE::self().free_ep(sndep);
    VPE::self().free_ep(rcvep);
}

static void cmds_write() {
    const epid_t rcvep = VPE::self().alloc_ep();
    const epid_t sndep = VPE::self().alloc_ep();
    DTU &dtu = DTU::get();

    void *addr = map_page();
    if(!addr)
        return;

    cout << "-- Test errors --\n";
    {
        word_t data[] = {1234, 5678, 1122, 3344};
        dtu.configure(sndep, reinterpret_cast<word_t>(addr) | MemGate::W, env()->pe,
            rcvep, sizeof(data), 0);

        dmacmd(nullptr, 0, sndep, 0, sizeof(data), DTU::READ);
        WVASSERTEQ(dtu.get_cmd(DTU::CMD_ERROR), static_cast<word_t>(Errors::NO_PERM));
    }

    cout << "-- Test writing --\n";
    {
        word_t data[] = {1234, 5678, 1122, 3344};
        dtu.configure(sndep, reinterpret_cast<word_t>(addr) | MemGate::W, env()->pe,
            rcvep, sizeof(data), 0);

        dmacmd(data, sizeof(data), sndep, 0, sizeof(data), DTU::WRITE);
        WVASSERTEQ(dtu.get_cmd(DTU::CMD_ERROR), static_cast<word_t>(Errors::NONE));
        volatile const word_t *words = reinterpret_cast<const word_t*>(addr);
        for(size_t i = 0; i < sizeof(data) / sizeof(data[0]); ++i)
            WVASSERTEQ(static_cast<word_t>(words[i]), data[i]);
    }

    unmap_page(addr);
    dtu.configure(sndep, 0, 0, 0, 0, 0);
    VPE::self().free_ep(sndep);
    VPE::self().free_ep(rcvep);
}

static void mem_sync() {
    static xfer_t data[4];

    MemGate mem = m3::MemGate::create_global(0x4000, m3::MemGate::RWX);
    MemGate gate = MemGate::bind(mem.sel());

    cout << "-- Test read sync --\n";
    {
        write_vmsg(gate, 0, 1, 2, 3, 4);
        gate.read(data, sizeof(data), 0);
        WVASSERTEQ(data[0], 1u);
        WVASSERTEQ(data[1], 2u);
        WVASSERTEQ(data[2], 3u);
        WVASSERTEQ(data[3], 4u);
    }
}

static void mem_derive() {
    static xfer_t test[6] = {0};

    MemGate mem = m3::MemGate::create_global(0x4000, m3::MemGate::RWX);
    MemGate gate = MemGate::bind(mem.sel());
    write_vmsg(gate, 0, 1, 2, 3, 4);

    cout << "-- Test derive --\n";
    {
        gate.read(test, sizeof(xfer_t) * 4, 0);

        WVASSERTEQ(test[0], 1u);
        WVASSERTEQ(test[1], 2u);
        WVASSERTEQ(test[2], 3u);
        WVASSERTEQ(test[3], 4u);
        WVASSERTEQ(test[4], 0u);

        MemGate sub = gate.derive(4 * sizeof(xfer_t), sizeof(xfer_t), MemGate::RWX);
        write_vmsg(sub, 0, 5);
        gate.read(test, sizeof(xfer_t) * 5, 0);

        WVASSERTEQ(test[0], 1u);
        WVASSERTEQ(test[1], 2u);
        WVASSERTEQ(test[2], 3u);
        WVASSERTEQ(test[3], 4u);
        WVASSERTEQ(test[4], 5u);
    }

    cout << "-- Test wrong derive --\n";
    {
        MemGate sub = gate.derive(4 * sizeof(xfer_t), sizeof(xfer_t), MemGate::R);
        sub.read(test, sizeof(xfer_t), 0);
        WVASSERTEQ(test[0], 5u);

        WVASSERTERR(Errors::NO_PERM, [&sub] { write_vmsg(sub, 0, 8); });
    }
}

void tdtu() {
    RUN_TEST(cmds_read);
    RUN_TEST(cmds_write);
    RUN_TEST(mem_sync);
    RUN_TEST(mem_derive);
}

#endif
