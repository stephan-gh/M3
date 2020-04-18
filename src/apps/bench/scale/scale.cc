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
#include <base/util/Time.h>
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
    explicit App(size_t argc, const char **argv)
        : argc(argc),
          argv(argv),
          pe(PE::alloc(VPE::self().pe_desc())),
          vpe(pe, argv[0]),
          rgate(RecvGate::create_for(vpe, 6, 6)),
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
    cerr << "Usage: " << name << " [-l] [-i <instances>] [-s <servers>] [-r <repeats>] <name>\n";
    cerr << "  -l enables the load generator\n";
    cerr << "  <instances> specifies the number of application (<name>) instances\n";
    cerr << "  <servers> specifies the number of m3fs instances\n";
    cerr << "  <repeats> specifies the number of repetitions of the benchmark\n";
    cerr << "  <name> specifies the name of the application trace\n";
    exit(1);
}

int main(int argc, char **argv) {
    bool loadgen = false;
    size_t instances = 1;
    size_t servers = 1;
    int repeats = 1;

    int opt;
    while((opt = CmdArgs::get(argc, argv, "li:s:r:")) != -1) {
        switch(opt) {
            case 'l': loadgen = true; break;
            case 'i': instances = IStringStream::read_from<size_t>(CmdArgs::arg); break;
            case 's': servers = IStringStream::read_from<size_t>(CmdArgs::arg); break;
            case 'r': repeats = IStringStream::read_from<int>(CmdArgs::arg); break;
            default:
                usage(argv[0]);
        }
    }
    if(CmdArgs::ind >= argc)
        usage(argv[0]);

    const char *name = argv[CmdArgs::ind + 0];

    App *apps[instances];
    Reference<PE> srvpes[servers];

    if(VERBOSE) cout << "Creating application VPEs...\n";

    const size_t ARG_COUNT = loadgen ? 9 : 7;
    for(size_t i = 0; i < instances; ++i) {
        const char **args = new const char *[ARG_COUNT];
        args[0] = "/bin/fstrace-m3fs";

        apps[i] = new App(ARG_COUNT, args);
    }

    if(VERBOSE) cout << "Starting VPEs...\n";

    for(size_t i = 0; i < instances; ++i) {
        OStringStream tmpdir(new char[16], 16);
        tmpdir << "/tmp/" << i << "/";
        const char **args = apps[i]->argv;
        if(repeats > 1) {
            args[1] = "-n";
            OStringStream num(new char[16], 16);
            num << repeats;
            args[2] = num.str();
        }
        else {
            args[1] = "-p";
            args[2] = tmpdir.str();
        }
        args[3] = "-w";
        args[4] = "-g";

        OStringStream rgatesel(new char[11], 11);
        rgatesel << apps[i]->rgate.sel();
        args[5] = rgatesel.str();
        if(loadgen) {
            args[6] = "-l";
            OStringStream loadgen(new char[16], 16);
            loadgen << "loadgen" << (i % 8);
            args[7] = loadgen.str();
            args[8] = name;
        }
        else
            args[6] = name;

        if(VERBOSE) {
            cout << "Starting ";
            for(size_t x = 0; x < ARG_COUNT; ++x)
                cout << args[x] << " ";
            cout << "\n";
        }

        apps[i]->vpe.mounts(VPE::self().mounts());
        apps[i]->vpe.obtain_mounts();

        apps[i]->vpe.exec(static_cast<int>(apps[i]->argc), apps[i]->argv);
    }

    if(VERBOSE) cout << "Signaling VPEs...\n";

    for(size_t i = 0; i < instances; ++i)
        send_receive_vmsg(apps[i]->sgate, 1);
    for(size_t i = 0; i < instances; ++i)
        send_vmsg(apps[i]->sgate, 1);

    cycles_t start = Time::start(0x1234);

    if(VERBOSE) cout << "Waiting for VPEs...\n";

    int exitcode = 0;
    for(size_t i = 0; i < instances; ++i) {
        int res = apps[i]->vpe.wait();
        if(res != 0)
            exitcode = 1;
        if(VERBOSE) cout << apps[i]->argv[0] << " exited with " << res << "\n";
    }

    cycles_t end = Time::stop(0x1234);
    cout << "Time: " << (end - start) << "\n";

    if(VERBOSE) cout << "Deleting VPEs...\n";

    for(size_t i = 0; i < instances; ++i)
        delete apps[i];

    if(VERBOSE) cout << "Done\n";
    return exitcode;
}
