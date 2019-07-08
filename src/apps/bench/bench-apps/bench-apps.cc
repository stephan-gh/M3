/**
 * Copyright (C) 2015, René Küttner <rene.kuettner@.tu-dresden.de>
 * Copyright (C) 2016-2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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
#include <base/stream/IStringStream.h>
#include <base/util/Time.h>
#include <base/CmdArgs.h>
#include <base/Panic.h>

#include <m3/stream/Standard.h>
#include <m3/vfs/Dir.h>
#include <m3/vfs/VFS.h>
#include <m3/Syscalls.h>
#include <m3/VPE.h>

using namespace m3;

#define VERBOSE         0
#define MAX_TMP_DIRS    4
#define MAX_TMP_FILES   16

struct App {
    explicit App(int argc, const char *argv[], bool muxed)
        : argc(argc),
          argv(argv),
          vpe(argv[0], VPEArgs().flags(muxed ? VPE::MUXABLE : 0)) {
    }

    int argc;
    const char **argv;
    VPE vpe;
};

static void usage(const char *name) {
    cerr << "Usage: " << name << " [-m <muxed>] [-r <repeats>] <argcount> <prog1>...\n";
    cerr << "  <muxed> specifies whether the PEs are multiplexed\n";
    cerr << "  <repeats> specifies the number of repetitions of the benchmark\n";
    exit(1);
}

int main(int argc, char **argv) {
    bool muxed = false;
    int repeats = 1;

    int opt;
    while((opt = CmdArgs::get(argc, argv, "m:r:")) != -1) {
        switch(opt) {
            case 'm': muxed = IStringStream::read_from<int>(CmdArgs::arg) == 1; break;
            case 'r': repeats = IStringStream::read_from<int>(CmdArgs::arg); break;
            default:
                usage(argv[0]);
        }
    }
    if(CmdArgs::ind >= argc)
        usage(argv[0]);

    int argcount = IStringStream::read_from<int>(argv[CmdArgs::ind]);
    App *apps[static_cast<size_t>(argc - (CmdArgs::ind + 1)) / static_cast<size_t>(argcount)];

    for(int j = 0; j < repeats; ++j) {
        if(VERBOSE) cout << "Creating VPEs...\n";

        size_t idx = 0;
        for(int i = CmdArgs::ind + 1; i < argc; i += argcount) {
            const char **args = new const char*[argcount];
            for(int x = 0; x < argcount; ++x)
                args[x] = argv[i + x];
            if(VERBOSE) {
                cout << "Creating ";
                for(int x = 0; x < argcount; ++x)
                    cout << args[x] << " ";
                cout << "\n";
            }
            apps[idx++] = new App(argcount, args, muxed);
        }

        if(VERBOSE) cout << "Starting VPEs...\n";

        cycles_t start = Time::start(0x1234);

        for(size_t i = 0; i < ARRAY_SIZE(apps); ++i) {
            apps[i]->vpe.mounts(*VPE::self().mounts());
            apps[i]->vpe.obtain_mounts();
            apps[i]->vpe.exec(apps[i]->argc, apps[i]->argv);

            if(!muxed) {
                if(VERBOSE) cout << "Waiting for VPE " << apps[i]->argv[0] << " ...\n";

                UNUSED int res = apps[i]->vpe.wait();
                if(VERBOSE) cout << apps[i]->argv[0] << " exited with " << res << "\n";
            }
        }

        if(muxed) {
            if(VERBOSE) cout << "Waiting for VPEs...\n";

            for(size_t i = 0; i < ARRAY_SIZE(apps); ++i) {
                int res = apps[i]->vpe.wait();
                if(VERBOSE) cout << apps[i]->argv[0] << " exited with " << res << "\n";
            }
        }

        cycles_t end = Time::stop(0x1234);
        cout << "Time: " << (end - start) << "\n";

        if(VERBOSE) cout << "Deleting VPEs...\n";

        for(size_t i = 0; i < ARRAY_SIZE(apps); ++i)
            delete apps[i];

        if(VERBOSE) cout << "Cleaning up /tmp...\n";

        for(int i = 0; i < MAX_TMP_DIRS; ++i) {
            char path[128];
            OStringStream os(path, sizeof(path));
            os << "/tmp/" << i;

            try {
                Dir dir(os.str());

                size_t x = 0;
                String *entries[MAX_TMP_FILES];

                if(VERBOSE) cout << "Collecting files in " << os.str() << "\n";

                // remove all entries; we assume here that they are files
                Dir::Entry e;
                while(dir.readdir(e)) {
                    if(strcmp(e.name, ".") == 0 || strcmp(e.name, "..") == 0)
                        continue;

                    OStringStream file(path, sizeof(path));
                    file << "/tmp/" << i << "/" << e.name;
                    if(x > ARRAY_SIZE(entries))
                        PANIC("Too few entries");
                    entries[x++] = new String(file.str());
                }

                for(; x > 0; --x) {
                    if(VERBOSE) cout << "Unlinking " << *(entries[x - 1]) << "\n";
                    VFS::unlink(entries[x - 1]->c_str());
                    delete entries[x - 1];
                }
            }
            catch(...) {
                // ignore
            }
        }
    }

    if(VERBOSE) cout << "Done\n";
    return 0;
}
