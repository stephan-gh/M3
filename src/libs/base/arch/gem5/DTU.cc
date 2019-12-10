/*
 * Copyright (C) 2016-2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <base/util/Math.h>
#include <base/CPU.h>
#include <base/DTU.h>
#include <base/Init.h>
#include <base/KIF.h>

namespace m3 {

INIT_PRIO_DTU DTU DTU::inst;

void DTU::print(const char *str, size_t len) {
    uintptr_t buffer = buffer_addr();
    const reg_t *rstr = reinterpret_cast<const reg_t*>(str);
    const reg_t *end = reinterpret_cast<const reg_t*>(str + len);
    while(rstr < end) {
        CPU::write8b(buffer, *rstr);
        buffer += sizeof(reg_t);
        rstr++;
    }

    write_reg(CmdRegs::COMMAND, build_command(0, CmdOpCode::PRINT, 0, len));
}

Errors::Code DTU::send(epid_t ep, const void *msg, size_t size, label_t replylbl, epid_t reply_ep) {
    static_assert(KIF::Perm::R == DTU::R, "DTU::R does not match KIF::Perm::R");
    static_assert(KIF::Perm::W == DTU::W, "DTU::W does not match KIF::Perm::W");

    static_assert(KIF::Perm::R == DTU::PTE_R, "DTU::PTE_R does not match KIF::Perm::R");
    static_assert(KIF::Perm::W == DTU::PTE_W, "DTU::PTE_W does not match KIF::Perm::W");
    static_assert(KIF::Perm::X == DTU::PTE_X, "DTU::PTE_X does not match KIF::Perm::X");

    write_reg(CmdRegs::DATA, reinterpret_cast<reg_t>(msg) | (static_cast<reg_t>(size) << 32));
    if(replylbl)
        write_reg(CmdRegs::ARG1, replylbl);
    CPU::compiler_barrier();
    write_reg(CmdRegs::COMMAND, build_command(ep, CmdOpCode::SEND, 0, reply_ep));

    return get_error();
}

Errors::Code DTU::reply(epid_t ep, const void *reply, size_t size, const Message *msg) {
    assert(size <= 0xFFFFFFFF);
    write_reg(CmdRegs::DATA, reinterpret_cast<reg_t>(reply) | (static_cast<reg_t>(size) << 32));
    CPU::compiler_barrier();
    write_reg(CmdRegs::COMMAND, build_command(ep, CmdOpCode::REPLY, 0, reinterpret_cast<reg_t>(msg)));

    return get_error();
}

Errors::Code DTU::read(epid_t ep, void *data, size_t size, goff_t off, uint flags) {
    assert(size <= 0xFFFFFFFF);
    write_reg(CmdRegs::DATA, reinterpret_cast<reg_t>(data) | (static_cast<reg_t>(size) << 32));
    write_reg(CmdRegs::ARG1, off);
    CPU::compiler_barrier();
    write_reg(CmdRegs::COMMAND, build_command(ep, CmdOpCode::READ, flags));
    Errors::Code res = get_error();
    CPU::memory_barrier();
    return res;
}

Errors::Code DTU::write(epid_t ep, const void *data, size_t size, goff_t off, uint flags) {
    write_reg(CmdRegs::DATA, reinterpret_cast<reg_t>(data) | (static_cast<reg_t>(size) << 32));
    write_reg(CmdRegs::ARG1, off);
    CPU::compiler_barrier();
    write_reg(CmdRegs::COMMAND, build_command(ep, CmdOpCode::WRITE, flags));
    return get_error();
}

}
