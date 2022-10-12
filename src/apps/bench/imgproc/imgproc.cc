/*
 * Copyright (C) 2017-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2021 Nils Asmussen, Barkhausen Institut
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

#include "imgproc.h"

#include <base/Common.h>
#include <base/stream/IStringStream.h>
#include <base/time/Profile.h>

#include <m3/Test.h>
#include <m3/stream/Standard.h>
#include <m3/vfs/VFS.h>

#include <stdlib.h>
#include <unistd.h>

using namespace m3;

// the time for one 2048 block for 2D-FFT; determined by ALADDIN and
// picking the sweet spot between area, power and performance.
// 732 cycles for the FFT function. we have two loops in FFT2D with
// 16 iterations each. we unroll both 4 times, leading to
// (4 + 4) * 732 = 5856.

const CycleDuration ACCEL_TIMES[] = {
    CycleDuration::from_raw(5856 / 2), // FFT
    CycleDuration::from_raw(1189 / 2), // multiply
    CycleDuration::from_raw(5856 / 2), // IFFT
};

static void usage(const char *name) {
    eprintln("Usage: {} [-m <mode>] [-n <num>] [-w <warmups>] [-r <repeats>] <in>"_cf, name);
    eprintln("  <mode> can be:"_cf);
    eprintln("    'indir'      for a single chain, assisted"_cf);
    eprintln("    'dir'        for a single chain, connected directly"_cf);
    eprintln("    'dir-simple' for a single chain, connected via pipes"_cf);
    eprintln("  <num> specifies the number of chains"_cf);
    eprintln("  <warmups> specifies the number of warmups"_cf);
    eprintln("  <repeats> specifies the number of repetitions of the benchmark"_cf);
    exit(1);
}

int main(int argc, char **argv) {
    Mode mode = Mode::INDIR;
    const char *modename = "indir";
    size_t num = 1;
    ulong repeats = 1;
    ulong warmup = 1;

    int opt;
    while((opt = getopt(argc, argv, "m:n:r:w:")) != -1) {
        switch(opt) {
            case 'm': {
                modename = optarg;
                if(strcmp(optarg, "indir") == 0)
                    mode = Mode::INDIR;
                else if(strcmp(optarg, "dir") == 0)
                    mode = Mode::DIR;
                else if(strcmp(optarg, "dir-simple") == 0)
                    mode = Mode::DIR_SIMPLE;
                else
                    usage(argv[0]);
                break;
            }
            case 'n': num = IStringStream::read_from<size_t>(optarg); break;
            case 'r': repeats = IStringStream::read_from<ulong>(optarg); break;
            case 'w': warmup = IStringStream::read_from<ulong>(optarg); break;
            default: usage(argv[0]);
        }
    }
    if(optind >= argc)
        usage(argv[0]);

    const char *in = argv[optind];

    Results<CycleDuration> res(repeats);
    for(ulong i = 0; i < repeats + warmup; ++i) {
        CycleDuration time;
        if(mode == Mode::INDIR)
            time = chain_indirect(in, num);
        else
            time = chain_direct(in, num, mode);

        if(i >= warmup)
            res.push(time);
    }

    OStringStream os;
    format_to(os, "imgproc-{} ({} chains)"_cf, modename, num);
    WVPERF(os.str(), res);
    return 0;
}
