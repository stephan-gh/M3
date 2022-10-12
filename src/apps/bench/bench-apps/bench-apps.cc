/**
 * Copyright (C) 2015, René Küttner <rene.kuettner@.tu-dresden.de>
 * Copyright (C) 2016-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
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
#include <string>
#include <unistd.h>
#include <vector>

using namespace m3;

static constexpr bool VERBOSE = false;
static constexpr int MAX_TMP_DIRS = 4;

struct App {
    explicit App(int argc, const char *argv[])
        : argc(argc),
          argv(argv),
          tile(Tile::get("core")),
          act(tile, argv[0]) {
    }

    int argc;
    const char **argv;
    Reference<Tile> tile;
    ChildActivity act;
};

static void usage(const char *name) {
    eprintln("Usage: {} [-r <repeats>] <argcount> <prog1>..."_cf, name);
    eprintln("    <repeats> specifies the number of repetitions of the benchmark"_cf);
    exit(1);
}

int main(int argc, char **argv) {
    int repeats = 1;

    int opt;
    while((opt = getopt(argc, argv, "r:")) != -1) {
        switch(opt) {
            case 'r': repeats = IStringStream::read_from<int>(optarg); break;
            default: usage(argv[0]);
        }
    }
    if(optind >= argc)
        usage(argv[0]);

    size_t argcount = IStringStream::read_from<size_t>(argv[optind]);
    size_t totalargs = static_cast<size_t>(argc);
    size_t appcount = static_cast<size_t>(argc - (optind + 1)) / argcount;

    for(int j = 0; j < repeats; ++j) {
        if(VERBOSE)
            println("Creating activities..."_cf);

        {
            size_t idx = 0;
            std::unique_ptr<App> apps[appcount];

            for(size_t i = static_cast<size_t>(optind + 1); i < totalargs; i += argcount) {
                const char **args = new const char *[argcount];
                for(size_t x = 0; x < argcount; ++x)
                    args[x] = argv[i + x];
                if(VERBOSE) {
                    print("Creating "_cf);
                    for(size_t x = 0; x < argcount; ++x)
                        print("{:x} "_cf, args[x]);
                    println();
                }
                apps[idx++] = std::make_unique<App>(argcount, args);
            }

            if(VERBOSE)
                println("Starting activities..."_cf);

            auto start = CycleInstant::now();

            for(size_t i = 0; i < ARRAY_SIZE(apps); ++i) {
                apps[i]->act.add_mount("/", "/");
                apps[i]->act.exec(apps[i]->argc, apps[i]->argv);

                if(VERBOSE)
                    println("Waiting for Activity {}..."_cf, apps[i]->argv[0]);

                UNUSED int res = apps[i]->act.wait();
                if(VERBOSE)
                    println("{} exited with {}"_cf, apps[i]->argv[0], res);
            }

            auto end = CycleInstant::now();
            println("Time: {}"_cf, end.duration_since(start));

            if(VERBOSE)
                println("Deleting activities..."_cf);
        }

        if(VERBOSE)
            println("Cleaning up /tmp..."_cf);

        for(int i = 0; i < MAX_TMP_DIRS; ++i) {
            char path[128];
            OStringStream os(path, sizeof(path));
            format_to(os, "/tmp/{}"_cf, i);

            try {
                Dir dir(os.str());

                std::vector<std::string> entries;

                if(VERBOSE)
                    println("Collecting files in {}"_cf, os.str());

                // remove all entries; we assume here that they are files
                Dir::Entry e;
                while(dir.readdir(e)) {
                    if(strcmp(e.name, ".") == 0 || strcmp(e.name, "..") == 0)
                        continue;

                    OStringStream file(path, sizeof(path));
                    format_to(file, "/tmp/{}/{}"_cf, i, e.name);
                    entries.push_back(file.str());
                }

                for(std::string &s : entries) {
                    if(VERBOSE)
                        println("Unlinking {}"_cf, s);
                    VFS::unlink(s.c_str());
                }
            }
            catch(...) {
                // ignore
            }
        }
    }

    if(VERBOSE)
        println("Done"_cf);
    return 0;
}
