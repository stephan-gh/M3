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

#include <m3/server/RemoteServer.h>
#include <m3/stream/Standard.h>
#include <m3/vfs/Dir.h>
#include <m3/vfs/VFS.h>
#include <m3/Syscalls.h>
#include <m3/VPE.h>

using namespace m3;

static constexpr bool VERBOSE = true;

struct App {
    explicit App(size_t argc, const char **argv, const char *pager)
        : argc(argc),
          argv(argv),
          vpe(argv[0], VPEArgs().pager(pager)),
          rgate(RecvGate::create_for(vpe, 6, 6)),
          sgate(SendGate::create(&rgate)) {
        vpe.delegate_obj(rgate.sel());
    }

    size_t argc;
    const char **argv;
    VPE vpe;
    RecvGate rgate;
    SendGate sgate;
};

static void usage(const char *name) {
    cerr << "Usage: " << name << " [-l] [-i <instances>] [-s <servers>] [-r <repeats>] <name> <fssize>\n";
    cerr << "  -l enables the load generator\n";
    cerr << "  <instances> specifies the number of application (<name>) instances\n";
    cerr << "  <servers> specifies the number of m3fs instances\n";
    cerr << "  <repeats> specifies the number of repetitions of the benchmark\n";
    cerr << "  <name> specifies the name of the application trace\n";
    cerr << "  <fssize> specifies the size of the file system\n";
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
    if(CmdArgs::ind + 1 >= argc)
        usage(argv[0]);

    const char *name = argv[CmdArgs::ind + 0];
    size_t fssize = IStringStream::read_from<size_t>(argv[CmdArgs::ind + 1]);

    App *apps[instances];
    RemoteServer *srv[1 + servers];
    VPE *srvvpes[1 + servers];
    char srvnames[1 + servers][16];

#if defined(__gem5__)
    if(VERBOSE) cout << "Creating pager...\n";

    {
        srvvpes[0] = new VPE("pager");
        srv[0] = new RemoteServer(*srvvpes[0], "mypager");
        OStringStream pager_name(srvnames[0], sizeof(srvnames[0]));
        pager_name << "pager";

        String srv_arg = srv[0]->sel_arg();
        const char *args[] = {"/bin/pager", "-a", "16", "-f", "16", "-s", srv_arg.c_str()};
        srvvpes[0]->exec(ARRAY_SIZE(args), args);
    }
#else
    srvvpes[0] = nullptr;
    srv[0] = nullptr;
#endif

    if(VERBOSE) cout << "Creating application VPEs...\n";

    const size_t ARG_COUNT = loadgen ? 11 : 9;
    for(size_t i = 0; i < instances; ++i) {
        const char **args = new const char *[ARG_COUNT];
        args[0] = "/bin/fstrace-m3fs";

        apps[i] = new App(ARG_COUNT, args, "mypager");
    }

    if(VERBOSE) cout << "Creating servers...\n";

    for(size_t i = 0; i < servers; ++i) {
        srvvpes[i + 1] = new VPE("m3fs");
        OStringStream m3fs_name(srvnames[i + 1], sizeof(srvnames[i + 1]));
        m3fs_name << "m3fs" << i;
        srv[i + 1] = new RemoteServer(*srvvpes[i + 1], m3fs_name.str());

        String m3fsarg = srv[i + 1]->sel_arg();
        OStringStream fs_off_str(new char[16], 16);
        fs_off_str << (fssize * i);
        OStringStream fs_size_str(new char[16], 16);
        fs_size_str << fssize;
        const char *m3fs_args[] = {
            "/bin/m3fs",
            "-n", srvnames[i + 1],
            "-s", m3fsarg.c_str(),
            "-o", fs_off_str.str(),
            "-e", "512",
            "mem",
            fs_size_str.str()
        };
        if(VERBOSE) {
            cout << "Creating ";
            for(size_t x = 0; x < ARRAY_SIZE(m3fs_args); ++x)
                cout << m3fs_args[x] << " ";
            cout << "\n";
        }
        srvvpes[i + 1]->exec(ARRAY_SIZE(m3fs_args), m3fs_args);
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
        args[4] = "-f";
        args[5] = srvnames[1 + (i % servers)];
        args[6] = "-g";

        OStringStream rgatesel(new char[11], 11);
        rgatesel << apps[i]->rgate.sel();
        args[7] = rgatesel.str();
        if(loadgen) {
            args[8] = "-l";
            OStringStream loadgen(new char[16], 16);
            loadgen << "loadgen" << (i % 8);
            args[9] = loadgen.str();
            args[10] = name;
        }
        else
            args[8] = name;

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

    for(size_t i = 0; i < instances; ++i) {
        int res = apps[i]->vpe.wait();
        if(VERBOSE) cout << apps[i]->argv[0] << " exited with " << res << "\n";
    }

    cycles_t end = Time::stop(0x1234);
    cout << "Time: " << (end - start) << "\n";

    if(VERBOSE) cout << "Deleting VPEs...\n";

    for(size_t i = 0; i < instances; ++i)
        delete apps[i];

    if(VERBOSE) cout << "Shutting down servers...\n";

    for(size_t i = 0; i < servers + 1; ++i) {
        if(!srv[i])
            continue;
        srv[i]->request_shutdown();
        int res = srvvpes[i]->wait();
        if(VERBOSE) cout << srvnames[i] << " exited with " << res << "\n";
    }
    for(size_t i = 0; i < servers + 1; ++i) {
        delete srv[i];
        delete srvvpes[i];
    }

    if(VERBOSE) cout << "Done\n";
    return 0;
}
