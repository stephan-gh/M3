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
#include <base/stream/IStringStream.h>
#include <base/util/Time.h>
#include <base/CmdArgs.h>

#include <m3/stream/Standard.h>
#include <m3/vfs/VFS.h>

#include "imgproc.h"

using namespace m3;

const cycles_t ACCEL_TIMES[] = {
    5856 / 2,   // FFT
    1189 / 2,   // multiply
    5856 / 2,   // IFFT
};

static void usage(const char *name) {
    cerr << "Usage: " << name << " [-m <mode>] [-n <num>] [-r <repeats>] <in>\n";
    cerr << "  <mode> can be:\n";
    cerr << "    'indir'      for a single chain, assisted\n";
    cerr << "    'dir'        for a single chain, connected directly\n";
    cerr << "    'dir-simple' for a single chain, connected via pipes\n";
    cerr << "  <num> specifies the number of chains\n";
    cerr << "  <repeats> specifies the number of repetitions of the benchmark\n";
    exit(1);
}

int main(int argc, char **argv) {
    Mode mode = Mode::INDIR;
    size_t num = 1;
    int repeats = 1;

    int opt;
    while((opt = CmdArgs::get(argc, argv, "m:n:r:")) != -1) {
        switch(opt) {
            case 'm': {
                if(strcmp(CmdArgs::arg, "indir") == 0)
                    mode = Mode::INDIR;
                else if(strcmp(CmdArgs::arg, "dir") == 0)
                    mode = Mode::DIR;
                else if(strcmp(CmdArgs::arg, "dir-simple") == 0)
                    mode = Mode::DIR_SIMPLE;
                else
                    usage(argv[0]);
                break;
            }
            case 'n': num = IStringStream::read_from<size_t>(CmdArgs::arg); break;
            case 'r': repeats = IStringStream::read_from<int>(CmdArgs::arg); break;
            default:
                usage(argv[0]);
        }
    }
    if(CmdArgs::ind >= argc)
        usage(argv[0]);

    const char *in = argv[CmdArgs::ind];

    for(int i = 0; i < repeats; ++i) {
        if(mode == Mode::INDIR)
            chain_indirect(in, num);
        else
            chain_direct(in, num, mode);
    }
    return 0;
}
