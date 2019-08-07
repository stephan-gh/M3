/**
 * Copyright (C) 2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universität Dresden (Germany)
 *
 * This file is part of M3 (Microkernel for Minimalist Manycores).
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

#include <base/DTU.h>

namespace m3 {

Errors::Code DTU::send(epid_t ep, const void *msg, size_t size, label_t replylbl, epid_t reply_ep) {
    write_reg(CmdRegs::DATA, reinterpret_cast<reg_t>(msg) | (static_cast<reg_t>(size) << 48));
    if(replylbl)
        write_reg(CmdRegs::REPLY_LABEL, replylbl);
    CPU::compiler_barrier();
    write_reg(CmdRegs::COMMAND, build_command(ep, CmdOpCode::SEND, 0, reply_ep));

    return get_error();
}

}
