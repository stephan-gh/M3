/**
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <base/Common.h>
#include <base/Panic.h>
#include <base/stream/IStringStream.h>
#include <base/time/Instant.h>

#include <m3/Syscalls.h>
#include <m3/stream/Standard.h>
#include <m3/tiles/ChildActivity.h>
#include <m3/vfs/Dir.h>
#include <m3/vfs/VFS.h>

#include <stdlib.h>
#include <unistd.h>

using namespace m3;

static constexpr bool VERBOSE = true;

struct App {
    explicit App(Reference<Tile> tile, size_t argc, const char **argv)
        : argc(argc),
          argv(argv),
          tile(tile),
          act(tile, argv[0]),
          rcap(RecvCap::create(6, 6)),
          sgate(SendGate::create(&rcap)) {
        act.delegate_obj(rcap.sel());
    }

    size_t argc;
    const char **argv;
    Reference<Tile> tile;
    ChildActivity act;
    RecvCap rcap;
    SendGate sgate;
};

static void usage(const char *name) {
    eprintln("Usage: {} [-l] [-i <instances>] [-r <repeats>] <name>"_cf, name);
    eprintln("  -l enables the load generator"_cf);
    eprintln("  <instances> specifies the number of application (<name>) instances"_cf);
    eprintln("  <repeats> specifies the number of repetitions of the benchmark"_cf);
    eprintln("  <name> specifies the name of the application trace"_cf);
    exit(1);
}

int main(int argc, char **argv) {
    bool loadgen = false;
    size_t instances = 1;
    int repeats = 1;

    int opt;
    while((opt = getopt(argc, argv, "li:s:r:f:")) != -1) {
        switch(opt) {
            case 'l': loadgen = true; break;
            case 'i': instances = IStringStream::read_from<size_t>(optarg); break;
            case 'r': repeats = IStringStream::read_from<int>(optarg); break;
            default: usage(argv[0]);
        }
    }
    if(optind >= argc)
        usage(argv[0]);

    const char *name = argv[optind];

    App *apps[instances];
    App *fs[instances];

    if(VERBOSE)
        println("Creating application activities..."_cf);

    const size_t ARG_COUNT = loadgen ? 11 : 9;
    const size_t FS_ARG_COUNT = 8;
    for(size_t i = 0; i < instances; ++i) {
        auto tile = Tile::get("core");

        {
            const char **args = new const char *[ARG_COUNT];
            args[0] = "/bin/fstrace-m3fs";
            apps[i] = new App(tile, ARG_COUNT, args);
        }

        {
            const char **args = new const char *[FS_ARG_COUNT];
            args[0] = "/sbin/m3fs";
            fs[i] = new App(tile, FS_ARG_COUNT, args);
        }
    }

    if(VERBOSE)
        println("Starting activities..."_cf);

    for(size_t i = 0; i < instances; ++i) {
        OStringStream inst_name, fs_name;
        fs[i]->argv[1] = "-m";
        fs[i]->argv[2] = "1";
        fs[i]->argv[3] = "-n";
        format_to(inst_name, "m3fs-{}"_cf, i);
        fs[i]->argv[4] = inst_name.str();
        fs[i]->argv[5] = "-f";
        format_to(fs_name, "fs{}"_cf, i + 1);
        fs[i]->argv[6] = fs_name.str();
        fs[i]->argv[7] = "mem";

        fs[i]->act.exec(static_cast<int>(fs[i]->argc), fs[i]->argv);

        // wait until the service is available
        while(true) {
            try {
                ClientSession sess(inst_name.str());
                break;
            }
            catch(...) {
                OwnActivity::sleep_for(TimeDuration::from_micros(10));
            }
        }

        OStringStream tmpdir(new char[16], 16);
        format_to(tmpdir, "/tmp/{}/"_cf, i);
        if(repeats > 1) {
            apps[i]->argv[1] = "-n";
            OStringStream num(new char[16], 16);
            format_to(num, "{}"_cf, repeats);
            apps[i]->argv[2] = num.str();
        }
        else {
            apps[i]->argv[1] = "-p";
            apps[i]->argv[2] = tmpdir.str();
        }
        apps[i]->argv[3] = "-w";
        apps[i]->argv[4] = "-g";

        OStringStream rgatesel(new char[11], 11);
        format_to(rgatesel, "{}"_cf, apps[i]->rcap.sel());
        apps[i]->argv[5] = rgatesel.str();
        apps[i]->argv[6] = "-f";
        apps[i]->argv[7] = inst_name.str();
        if(loadgen) {
            apps[i]->argv[8] = "-l";
            OStringStream loadgen(new char[16], 16);
            format_to(loadgen, "loadgen{}"_cf, i % 8);
            apps[i]->argv[9] = loadgen.str();
            apps[i]->argv[10] = name;
        }
        else
            apps[i]->argv[8] = name;

        if(VERBOSE) {
            print("Starting "_cf);
            for(size_t x = 0; x < ARG_COUNT; ++x)
                print("{} "_cf, apps[i]->argv[x]);
            println();
        }

        apps[i]->act.exec(static_cast<int>(apps[i]->argc), apps[i]->argv);
    }

    if(VERBOSE)
        println("Signaling activities..."_cf);

    for(size_t i = 0; i < instances; ++i)
        send_receive_vmsg(apps[i]->sgate, 1);
    for(size_t i = 0; i < instances; ++i)
        send_vmsg(apps[i]->sgate, 1);

    auto start = CycleInstant::now();

    if(VERBOSE)
        println("Waiting for activities..."_cf);

    int exitcode = 0;
    for(size_t i = 0; i < instances; ++i) {
        int res = apps[i]->act.wait();
        if(res != 0)
            exitcode = 1;
        if(VERBOSE)
            println("{} exited with {}"_cf, apps[i]->argv[0], res);
    }

    auto end = CycleInstant::now();
    println("Time: {}"_cf, end.duration_since(start));

    if(VERBOSE)
        println("Deleting activities..."_cf);

    for(size_t i = 0; i < instances; ++i) {
        delete apps[i];
        delete fs[i];
    }

    if(VERBOSE)
        println("Done"_cf);
    return exitcode;
}
