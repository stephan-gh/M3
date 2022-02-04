/**
 * Copyright (C) 2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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
#include <base/time/Instant.h>
#include <base/CmdArgs.h>
#include <base/Panic.h>

#include <m3/stream/Standard.h>
#include <m3/vfs/Dir.h>
#include <m3/vfs/VFS.h>
#include <m3/Syscalls.h>
#include <m3/pes/VPE.h>

using namespace m3;

static constexpr bool VERBOSE = true;

struct App {
    explicit App(Reference<PE> pe, size_t argc, const char **argv)
        : argc(argc),
          argv(argv),
          pe(pe),
          vpe(pe, argv[0]),
          rgate(RecvGate::create(6, 6)),
          sgate(SendGate::create(&rgate)) {
        vpe.delegate_obj(rgate.sel());
    }

    size_t argc;
    const char **argv;
    Reference<PE> pe;
    VPE vpe;
    RecvGate rgate;
    SendGate sgate;
};

static void usage(const char *name) {
    cerr << "Usage: " << name << " [-l] [-i <instances>] [-r <repeats>] [-f <fssize>] <name>\n";
    cerr << "  -l enables the load generator\n";
    cerr << "  <instances> specifies the number of application (<name>) instances\n";
    cerr << "  <repeats> specifies the number of repetitions of the benchmark\n";
    cerr << "  <name> specifies the name of the application trace\n";
    cerr << "  -f <fssize> creates an own m3fs instance for every application with given size\n";
    exit(1);
}

int main(int argc, char **argv) {
    bool loadgen = false;
    size_t instances = 1;
    int repeats = 1;
    const char *fs_size_str = nullptr;
    size_t fs_size = 0;

    int opt;
    while((opt = CmdArgs::get(argc, argv, "li:s:r:f:")) != -1) {
        switch(opt) {
            case 'l': loadgen = true; break;
            case 'i': instances = IStringStream::read_from<size_t>(CmdArgs::arg); break;
            case 'r': repeats = IStringStream::read_from<int>(CmdArgs::arg); break;
            case 'f': {
                fs_size_str = CmdArgs::arg;
                fs_size = IStringStream::read_from<size_t>(fs_size_str);
                break;
            }
            default:
                usage(argv[0]);
        }
    }
    if(CmdArgs::ind >= argc)
        usage(argv[0]);

    const char *name = argv[CmdArgs::ind];

    App *apps[instances];
    App *fs[instances];

    if(VERBOSE) cout << "Creating application VPEs...\n";

    const size_t ARG_COUNT = loadgen ? 11 : 9;
    const size_t FS_ARG_COUNT = 9;
    for(size_t i = 0; i < instances; ++i) {
        auto pe = PE::get("core");

        {
            const char **args = new const char *[ARG_COUNT];
            args[0] = "/bin/fstrace-m3fs";
            apps[i] = new App(pe, ARG_COUNT, args);
        }

        if(fs_size_str) {
            const char **args = new const char *[FS_ARG_COUNT];
            args[0] = "/sbin/m3fs";
            fs[i] = new App(pe, FS_ARG_COUNT, args);
        }
    }

    if(VERBOSE) cout << "Starting VPEs...\n";

    for(size_t i = 0; i < instances; ++i) {
        OStringStream fs_name;
        if(fs_size_str) {
            fs[i]->argv[1] = "-m";
            fs[i]->argv[2] = "1";
            fs[i]->argv[3] = "-o";
            OStringStream fs_off;
            fs_off << (i * fs_size);
            fs[i]->argv[4] = fs_off.str();
            fs[i]->argv[5] = "-n";
            fs_name << "m3fs-" << i;
            fs[i]->argv[6] = fs_name.str();
            fs[i]->argv[7] = "mem";
            fs[i]->argv[8] = fs_size_str;

            fs[i]->vpe.exec(static_cast<int>(fs[i]->argc), fs[i]->argv);

            // wait until the service is available
            while(true) {
                try {
                    ClientSession sess(fs_name.str());
                    break;
                }
                catch(...) {
                    VPE::self().sleep_for(TimeDuration::from_micros(10));
                }
            }
        }

        OStringStream tmpdir(new char[16], 16);
        tmpdir << "/tmp/" << i << "/";
        if(repeats > 1) {
            apps[i]->argv[1] = "-n";
            OStringStream num(new char[16], 16);
            num << repeats;
            apps[i]->argv[2] = num.str();
        }
        else {
            apps[i]->argv[1] = "-p";
            apps[i]->argv[2] = tmpdir.str();
        }
        apps[i]->argv[3] = "-w";
        apps[i]->argv[4] = "-g";

        OStringStream rgatesel(new char[11], 11);
        rgatesel << apps[i]->rgate.sel();
        apps[i]->argv[5] = rgatesel.str();
        if(fs_size_str) {
            apps[i]->argv[6] = "-f";
            apps[i]->argv[7] = fs_name.str();
        }
        else {
            apps[i]->argv[6] = "-w";
            apps[i]->argv[7] = "-w";
        }
        if(loadgen) {
            apps[i]->argv[8] = "-l";
            OStringStream loadgen(new char[16], 16);
            loadgen << "loadgen" << (i % 8);
            apps[i]->argv[9] = loadgen.str();
            apps[i]->argv[10] = name;
        }
        else
            apps[i]->argv[8] = name;

        if(VERBOSE) {
            cout << "Starting ";
            for(size_t x = 0; x < ARG_COUNT; ++x)
                cout << apps[i]->argv[x] << " ";
            cout << "\n";
        }

        if(!fs_size_str)
            apps[i]->vpe.mounts()->add("/", VPE::self().mounts()->get("/"));

        apps[i]->vpe.exec(static_cast<int>(apps[i]->argc), apps[i]->argv);
    }

    if(VERBOSE) cout << "Signaling VPEs...\n";

    for(size_t i = 0; i < instances; ++i)
        send_receive_vmsg(apps[i]->sgate, 1);
    for(size_t i = 0; i < instances; ++i)
        send_vmsg(apps[i]->sgate, 1);

    auto start = CycleInstant::now();

    if(VERBOSE) cout << "Waiting for VPEs...\n";

    int exitcode = 0;
    for(size_t i = 0; i < instances; ++i) {
        int res = apps[i]->vpe.wait();
        if(res != 0)
            exitcode = 1;
        if(VERBOSE) cout << apps[i]->argv[0] << " exited with " << res << "\n";
    }

    auto end = CycleInstant::now();
    cout << "Time: " << end.duration_since(start) << "\n";

    if(VERBOSE) cout << "Deleting VPEs...\n";

    for(size_t i = 0; i < instances; ++i) {
        delete apps[i];
        if(fs_size_str)
            delete fs[i];
    }

    if(VERBOSE) cout << "Done\n";
    return exitcode;
}
