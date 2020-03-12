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

#pragma once

#include <m3/session/ClientSession.h>
#include <m3/com/GateStream.h>
#include <m3/com/SendGate.h>
#include <m3/vfs/GenericFile.h>

namespace m3 {

class Pipes : public ClientSession {
public:
    class Pipe : public ClientSession {
    public:
        explicit Pipe(capsel_t sel, MemGate &memory)
            : ClientSession(sel) {
            delegate_obj(memory.sel());
        }
        Pipe(Pipe &&p) noexcept
            : ClientSession(std::move(p)) {
        }

        Reference<File> create_channel(bool read, int flags = 0) {
            KIF::ExchangeArgs args;
            ExchangeOStream os(args);
            os << read;
            args.bytes = os.total();
            KIF::CapRngDesc desc = obtain(2, &args);
            return Reference<File>(new GenericFile(flags | (read ? FILE_R : FILE_W), desc.start()));
        }
    };

    explicit Pipes(const String &service)
        : ClientSession(service) {
    }

    Pipe create_pipe(MemGate &memory, size_t memsize) {
        KIF::ExchangeArgs args;
        ExchangeOStream os(args);
        os << memsize;
        args.bytes = os.total();
        KIF::CapRngDesc desc = obtain(2, &args);
        return Pipe(desc.start(), memory);
    }
};

}
