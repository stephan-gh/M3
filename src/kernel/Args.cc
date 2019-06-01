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

#include <base/stream/IStringStream.h>
#include <base/stream/Serial.h>
#include <base/CmdArgs.h>
#include <base/Config.h>
#include <base/Machine.h>

#include "Args.h"

namespace kernel {

size_t Args::kmem          = 32 * 1024 * 1024;
cycles_t Args::timeslice   = 6000000;
const char *Args::fsimg    = nullptr;

void Args::usage(const char *name) {
    m3::Serial::get() << "Usage: " << name << " [-t=<timeslice>] [-f=<fsimg>] [-m=<kmem>] ...\n";
    m3::Serial::get() << "  -t: the timeslices for all VPEs\n";
    m3::Serial::get() << "  -f: the file system image (only used on host)\n";
    m3::Serial::get() << "  -m: the kernel memory size (> FIXED_KMEM)\n";
    m3::Machine::shutdown();
}

int Args::parse(int argc, char **argv) {
    int opt;
    while((opt = m3::CmdArgs::get(argc, argv, "f:t:m:")) != -1) {
        switch(opt) {
            case 'f': fsimg = m3::CmdArgs::arg; break;
            case 't': timeslice = m3::IStringStream::read_from<cycles_t>(m3::CmdArgs::arg); break;
            case 'm':
                kmem = m3::CmdArgs::to_size(m3::CmdArgs::arg);
                if(kmem <= FIXED_KMEM)
                    usage(argv[0]);
                break;
            default: usage(argv[0]);
        }
    }

    return m3::CmdArgs::ind;
}

}
