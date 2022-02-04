/*
 * Copyright (C) 2017-2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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
#include <base/CmdArgs.h>

#include <m3/stream/Standard.h>
#include <m3/vfs/VFS.h>

#include "accelchain.h"

using namespace m3;

static void usage(const char *name) {
    cerr << "Usage: " << name << " [-m <mode>] [-c <comptime>] [-n <num>] [-r <repeats>] <in> <out>\n";
    cerr << "  <mode> can be:\n";
    cerr << "    'indir'      for a single chain, assisted\n";
    cerr << "    'dir'        for a single chain, connected directly\n";
    cerr << "    'dir-simple' for a single chain, connected via pipes\n";
    cerr << "    'dir-multi'  for two chains, connected directly\n";
    cerr << "  <comptime> specifies the computation time for each accelerator for 1 KiB\n";
    cerr << "  <num> specifies the number of accelerators in each chain\n";
    cerr << "  <repeats> specifies the number of repetitions of the benchmark\n";
    exit(1);
}

int main(int argc, char **argv) {
    Mode mode = Mode::INDIR;
    CycleDuration comptime = CycleDuration::from_raw(1000);
    size_t num = 1;
    int repeats = 1;

    int opt;
    while((opt = CmdArgs::get(argc, argv, "m:c:n:r:")) != -1) {
        switch(opt) {
            case 'm': {
                if(strcmp(CmdArgs::arg, "indir") == 0)
                    mode = Mode::INDIR;
                else if(strcmp(CmdArgs::arg, "dir") == 0)
                    mode = Mode::DIR;
                else if(strcmp(CmdArgs::arg, "dir-simple") == 0)
                    mode = Mode::DIR_SIMPLE;
                else if(strcmp(CmdArgs::arg, "dir-multi") == 0)
                    mode = Mode::DIR_MULTI;
                else
                    usage(argv[0]);
                break;
            }
            case 'c': {
                auto cycles = IStringStream::read_from<cycles_t>(CmdArgs::arg);
                comptime = CycleDuration::from_raw(cycles);
                break;
            }
            case 'n': num = IStringStream::read_from<size_t>(CmdArgs::arg); break;
            case 'r': repeats = IStringStream::read_from<int>(CmdArgs::arg); break;
            default:
                usage(argv[0]);
        }
    }
    if(CmdArgs::ind + 1 >= argc)
        usage(argv[0]);

    const char *in = argv[CmdArgs::ind + 0];
    const char *out = argv[CmdArgs::ind + 1];

    for(int i = 0; i < repeats; ++i) {
        // open files
        fd_t infd = VFS::open(in, FILE_R | FILE_NEWSESS);
        fd_t outfd = VFS::open(out, FILE_W | FILE_TRUNC | FILE_CREATE | FILE_NEWSESS);

        auto fin = VPE::self().files()->get(infd);
        auto fout = VPE::self().files()->get(outfd);

        if(mode == Mode::INDIR)
            chain_indirect(fin, fout, num, comptime);
        else if(mode == Mode::DIR_MULTI)
            chain_direct_multi(fin, fout, num, comptime, Mode::DIR);
        else
            chain_direct(fin, fout, num, comptime, mode);

        VFS::close(infd);
        VFS::close(outfd);
    }
    return 0;
}
