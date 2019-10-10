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
#include <m3/pes/VPE.h>

#include <vector>

using namespace m3;

static constexpr bool VERBOSE           = false;
static constexpr int MAX_TMP_DIRS       = 4;

struct App {
    explicit App(int argc, const char *argv[])
        : argc(argc),
          argv(argv),
          pe(PE::alloc(VPE::self().pe_desc())),
          vpe(pe, argv[0]) {
    }

    int argc;
    const char **argv;
    PE pe;
    VPE vpe;
};

static void usage(const char *name) {
    cerr << "Usage: " << name << " [-r <repeats>] <argcount> <prog1>...\n";
    cerr << "  <repeats> specifies the number of repetitions of the benchmark\n";
    exit(1);
}

int main(int argc, char **argv) {
    int repeats = 1;

    int opt;
    while((opt = CmdArgs::get(argc, argv, "r:")) != -1) {
        switch(opt) {
            case 'r': repeats = IStringStream::read_from<int>(CmdArgs::arg); break;
            default:
                usage(argv[0]);
        }
    }
    if(CmdArgs::ind >= argc)
        usage(argv[0]);

    size_t argcount = IStringStream::read_from<size_t>(argv[CmdArgs::ind]);
    size_t totalargs = static_cast<size_t>(argc);
    size_t appcount = static_cast<size_t>(argc - (CmdArgs::ind + 1)) / argcount;

    for(int j = 0; j < repeats; ++j) {
        if(VERBOSE) cout << "Creating VPEs...\n";

        {
            size_t idx = 0;
            std::unique_ptr<App> apps[appcount];

            for(size_t i = static_cast<size_t>(CmdArgs::ind + 1); i < totalargs; i += argcount) {
                const char **args = new const char*[argcount];
                for(size_t x = 0; x < argcount; ++x)
                    args[x] = argv[i + x];
                if(VERBOSE) {
                    cout << "Creating ";
                    for(size_t x = 0; x < argcount; ++x)
                        cout << args[x] << " ";
                    cout << "\n";
                }
                apps[idx++] = std::make_unique<App>(argcount, args);
            }

            if(VERBOSE) cout << "Starting VPEs...\n";

            cycles_t start = Time::start(0x1234);

            for(size_t i = 0; i < ARRAY_SIZE(apps); ++i) {
                apps[i]->vpe.mounts(VPE::self().mounts());
                apps[i]->vpe.obtain_mounts();
                apps[i]->vpe.exec(apps[i]->argc, apps[i]->argv);

                if(VERBOSE) cout << "Waiting for VPE " << apps[i]->argv[0] << " ...\n";

                UNUSED int res = apps[i]->vpe.wait();
                if(VERBOSE) cout << apps[i]->argv[0] << " exited with " << res << "\n";
            }

            cycles_t end = Time::stop(0x1234);
            cout << "Time: " << (end - start) << "\n";

            if(VERBOSE) cout << "Deleting VPEs...\n";
        }

        if(VERBOSE) cout << "Cleaning up /tmp...\n";

        for(int i = 0; i < MAX_TMP_DIRS; ++i) {
            char path[128];
            OStringStream os(path, sizeof(path));
            os << "/tmp/" << i;

            try {
                Dir dir(os.str());

                std::vector<String> entries;

                if(VERBOSE) cout << "Collecting files in " << os.str() << "\n";

                // remove all entries; we assume here that they are files
                Dir::Entry e;
                while(dir.readdir(e)) {
                    if(strcmp(e.name, ".") == 0 || strcmp(e.name, "..") == 0)
                        continue;

                    OStringStream file(path, sizeof(path));
                    file << "/tmp/" << i << "/" << e.name;
                    entries.push_back(file.str());
                }

                for(String &s : entries) {
                    if(VERBOSE) cout << "Unlinking " << s << "\n";
                    VFS::unlink(s.c_str());
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
