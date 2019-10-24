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
#include <base/util/Profile.h>
#include <base/util/Time.h>
#include <base/CmdArgs.h>

#include <m3/stream/Standard.h>
#include <m3/vfs/VFS.h>
#include <m3/Test.h>

#include "imgproc.h"

using namespace m3;

// the time for one 2048 block for 2D-FFT; determined by ALADDIN and
// picking the sweet spot between area, power and performance.
// 732 cycles for the FFT function. we have two loops in FFT2D with
// 16 iterations each. we unroll both 4 times, leading to
// (4 + 4) * 732 = 5856.

const cycles_t ACCEL_TIMES[] = {
    5856 / 2,   // FFT
    1189 / 2,   // multiply
    5856 / 2,   // IFFT
};

static void usage(const char *name) {
    cerr << "Usage: " << name << " [-m <mode>] [-n <num>] [-w <warmups>] [-r <repeats>] <in>\n";
    cerr << "  <mode> can be:\n";
    cerr << "    'indir'      for a single chain, assisted\n";
    cerr << "    'dir'        for a single chain, connected directly\n";
    cerr << "    'dir-simple' for a single chain, connected via pipes\n";
    cerr << "  <num> specifies the number of chains\n";
    cerr << "  <warmups> specifies the number of warmups\n";
    cerr << "  <repeats> specifies the number of repetitions of the benchmark\n";
    exit(1);
}

int main(int argc, char **argv) {
    Mode mode = Mode::INDIR;
    const char *modename = "indir";
    size_t num = 1;
    ulong repeats = 1;
    ulong warmup = 1;

    int opt;
    while((opt = CmdArgs::get(argc, argv, "m:n:r:w:")) != -1) {
        switch(opt) {
            case 'm': {
                modename = CmdArgs::arg;
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
            case 'r': repeats = IStringStream::read_from<ulong>(CmdArgs::arg); break;
            case 'w': warmup = IStringStream::read_from<ulong>(CmdArgs::arg); break;
            default:
                usage(argv[0]);
        }
    }
    if(CmdArgs::ind >= argc)
        usage(argv[0]);

    const char *in = argv[CmdArgs::ind];

    Results res(repeats);
    for(ulong i = 0; i < repeats + warmup; ++i) {
        cycles_t time;
        if(mode == Mode::INDIR)
            time = chain_indirect(in, num);
        else
            time = chain_direct(in, num, mode);

        if(i >= warmup)
            res.push(time);
    }

    OStringStream os;
    os << "imgproc-" << modename << " (" << num << " chains)";
    WVPERF(os.str(), res);
    return 0;
}
