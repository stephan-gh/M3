/*
 * Copyright (C) 2017-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2022 Nils Asmussen, Barkhausen Institut
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

#include "accelchain.h"

#include <base/Common.h>
#include <base/stream/IStringStream.h>

#include <m3/stream/Standard.h>
#include <m3/vfs/VFS.h>

#include <stdlib.h>
#include <unistd.h>

using namespace m3;

static void usage(const char *name) {
    eprintln("Usage: {} [-m <mode>] [-c <comptime>] [-n <num>] [-r <repeats>] <in> <out>"_cf, name);
    eprintln("  <mode> can be:"_cf);
    eprintln("    'indir'      for a single chain, assisted"_cf);
    eprintln("    'dir'        for a single chain, connected directly"_cf);
    eprintln("    'dir-simple' for a single chain, connected via pipes"_cf);
    eprintln("    'dir-multi'  for two chains, connected directly"_cf);
    eprintln("  <comptime> specifies the computation time for each accelerator for 1 KiB"_cf);
    eprintln("  <num> specifies the number of accelerators in each chain"_cf);
    eprintln("  <repeats> specifies the number of repetitions of the benchmark"_cf);
    exit(1);
}

int main(int argc, char **argv) {
    Mode mode = Mode::INDIR;
    CycleDuration comptime = CycleDuration::from_raw(1000);
    size_t num = 1;
    int repeats = 1;

    int opt;
    while((opt = getopt(argc, argv, "m:c:n:r:")) != -1) {
        switch(opt) {
            case 'm': {
                if(strcmp(optarg, "indir") == 0)
                    mode = Mode::INDIR;
                else if(strcmp(optarg, "dir") == 0)
                    mode = Mode::DIR;
                else if(strcmp(optarg, "dir-simple") == 0)
                    mode = Mode::DIR_SIMPLE;
                else if(strcmp(optarg, "dir-multi") == 0)
                    mode = Mode::DIR_MULTI;
                else
                    usage(argv[0]);
                break;
            }
            case 'c': {
                auto cycles = IStringStream::read_from<cycles_t>(optarg);
                comptime = CycleDuration::from_raw(cycles);
                break;
            }
            case 'n': num = IStringStream::read_from<size_t>(optarg); break;
            case 'r': repeats = IStringStream::read_from<int>(optarg); break;
            default: usage(argv[0]);
        }
    }
    if(optind + 1 >= argc)
        usage(argv[0]);

    const char *in = argv[optind + 0];
    const char *out = argv[optind + 1];

    for(int i = 0; i < repeats; ++i) {
        // open files
        auto fin = VFS::open(in, FILE_R | FILE_NEWSESS);
        auto fout = VFS::open(out, FILE_W | FILE_TRUNC | FILE_CREATE | FILE_NEWSESS);

        if(mode == Mode::INDIR)
            chain_indirect(fin, fout, num, comptime);
        else if(mode == Mode::DIR_MULTI)
            chain_direct_multi(fin, fout, num, comptime, Mode::DIR);
        else
            chain_direct(fin, fout, num, comptime, mode);
    }
    return 0;
}
