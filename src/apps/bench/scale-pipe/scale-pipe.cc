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
#include <base/util/Profile.h>
#include <base/util/Time.h>
#include <base/CmdArgs.h>
#include <base/Panic.h>

#include <m3/server/RemoteServer.h>
#include <m3/stream/Standard.h>
#include <m3/pipe/IndirectPipe.h>
#include <m3/vfs/Dir.h>
#include <m3/vfs/VFS.h>
#include <m3/Syscalls.h>
#include <m3/Test.h>
#include <m3/VPE.h>

using namespace m3;

static constexpr bool VERBOSE = true;

struct App {
    explicit App(int argc, const char **argv, const char *pager)
        : argc(argc),
          argv(argv),
          vpe(argv[0], VPEArgs().pager(pager)),
          rgate(RecvGate::create_for(vpe, 6, 6)),
          sgate(SendGate::create(&rgate)) {
        rgate.activate();
        vpe.delegate_obj(rgate.sel());
    }

    int argc;
    const char **argv;
    VPE vpe;
    RecvGate rgate;
    SendGate sgate;
};

static void usage(const char *name) {
    cerr << "Usage: " << name << " [-d] [-i <instances>] [-r <repeats>] [-w <warmup>] <wr_name> <rd_name>\n";
    cerr << "  -d enables data transfers (otherwise the same time is spent locally)\n";
    cerr << "  <instances> specifies the number of application (<name>) instances\n";
    cerr << "  <repeats> specifies the number of repetitions of the benchmark\n";
    cerr << "  <warmup> specifies the number of warmup rounds\n";
    cerr << "  <wr_name> specifies the name of the application trace for the writer\n";
    cerr << "  <rd_name> specifies the name of the application trace for the reader\n";
    exit(1);
}

int main(int argc, char **argv) {
    bool data = false;
    size_t instances = 1;
    int repeats = 1;
    int warmup = 0;

    int opt;
    while((opt = CmdArgs::get(argc, argv, "di:r:w:")) != -1) {
        switch(opt) {
            case 'd': data = true; break;
            case 'i': instances = IStringStream::read_from<size_t>(CmdArgs::arg); break;
            case 'r': repeats = IStringStream::read_from<int>(CmdArgs::arg); break;
            case 'w': warmup = IStringStream::read_from<int>(CmdArgs::arg); break;
            default:
                usage(argv[0]);
        }
    }
    if(CmdArgs::ind + 1 >= argc)
        usage(argv[0]);

    const char *wr_name = argv[CmdArgs::ind + 0];
    const char *rd_name = argv[CmdArgs::ind + 1];

    App *apps[instances * 2];
    RemoteServer *srvs[3];
    VPE *srv_vpes[3];

#if defined(__gem5__)
    if(VERBOSE) cout << "Creating pager...\n";

    {
        srv_vpes[2] = new VPE("pager");
        srvs[2] = new RemoteServer(*srv_vpes[2], "mypager");

        String srv_arg = srvs[2]->sel_arg();
        const char *args[] = {"/bin/pager", "-a", "16", "-f", "16", "-s", srv_arg.c_str()};
        srv_vpes[2]->exec(ARRAY_SIZE(args), args);
    }
#else
    srv_vpes[2] = nullptr;
    srvs[2] = nullptr;
#endif

    if(VERBOSE) cout << "Creating application VPEs...\n";

    Results res(static_cast<ulong>(repeats));

    for(int j = 0; j < warmup + repeats; ++j) {
        const size_t ARG_COUNT = 11;
        for(size_t i = 0; i < instances * 2; ++i) {
            const char **args = new const char *[ARG_COUNT];
            args[0] = "/bin/fstrace-m3fs";

            apps[i] = new App(ARG_COUNT, args, "mypager");
        }

        if(j == 0 && VERBOSE) cout << "Creating servers...\n";

        if(j == 0) {
            srv_vpes[0] = new VPE("m3fs");
            srvs[0] = new RemoteServer(*srv_vpes[0], "mym3fs");

            String srv_arg = srvs[0]->sel_arg();
            const char *args[] = {"/bin/m3fs", "-s", srv_arg.c_str(), "mem", "268435456"};
            srv_vpes[0]->exec(ARRAY_SIZE(args), args);
        }

        if(j == 0) {
            srv_vpes[1] = new VPE("pipes");
            srvs[1] = new RemoteServer(*srv_vpes[1], "mypipe");

            String srv_arg = srvs[1]->sel_arg();
            const char *args[] = {"/bin/pipes", "-s", srv_arg.c_str()};
            srv_vpes[1]->exec(ARRAY_SIZE(args), args);
        }

        if(VERBOSE) cout << "Starting VPEs...\n";

        cycles_t overall_start = Time::start(0x1235);

        Pipes pipesrv("mypipe");
        constexpr size_t PIPE_SHM_SIZE   = 512 * 1024;
        MemGate *mems[instances];
        IndirectPipe *pipes[instances];

        for(size_t i = 0; i < instances * 2; ++i) {
            OStringStream tmpdir(new char[16], 16);
            tmpdir << "/tmp/" << i << "/";
            const char **args = apps[i]->argv;
            args[1] = "-p";
            args[2] = tmpdir.str();
            args[3] = instances > 1 ? "-w" : "-i";
            args[4] = "-i";
            args[5] = data ? "-d" : "-i";
            args[6] = "-f";
            args[7] = "mym3fs";
            args[8] = "-g";

            OStringStream rgatesel(new char[11], 11);
            rgatesel << apps[i]->rgate.sel() << " " << apps[i]->rgate.ep();
            args[9] = rgatesel.str();
            args[10] = (i % 2 == 0) ? wr_name : rd_name;

            if(VERBOSE) {
                cout << "Starting ";
                for(size_t x = 0; x < ARG_COUNT; ++x)
                    cout << args[x] << " ";
                cout << "\n";
            }

            if(i % 2 == 0) {
                mems[i / 2] = new MemGate(MemGate::create_global(PIPE_SHM_SIZE, MemGate::RW));
                pipes[i / 2] = new IndirectPipe(pipesrv, *mems[i / 2], PIPE_SHM_SIZE, data ? 0 : FILE_NODATA);
                apps[i]->vpe.fds()->set(STDOUT_FD, VPE::self().fds()->get(pipes[i / 2]->writer_fd()));
            }
            else
                apps[i]->vpe.fds()->set(STDIN_FD, VPE::self().fds()->get(pipes[i / 2]->reader_fd()));
            apps[i]->vpe.obtain_fds();

            apps[i]->vpe.mounts(VPE::self().mounts());
            apps[i]->vpe.obtain_mounts();

            apps[i]->vpe.exec(apps[i]->argc, apps[i]->argv);

            if(i % 2 == 1) {
                pipes[i / 2]->close_writer();
                pipes[i / 2]->close_reader();
            }
        }

        if(VERBOSE) cout << "Signaling VPEs...\n";

        for(size_t i = 0; i < instances * 2; ++i)
            send_receive_vmsg(apps[i]->sgate, 1);

        cycles_t start = Time::start(0x1234);

        for(size_t i = 0; i < instances * 2; ++i)
            send_vmsg(apps[i]->sgate, 1);

        if(VERBOSE) cout << "Waiting for VPEs...\n";

        for(size_t i = 0; i < instances * 2; ++i) {
            int res = apps[i]->vpe.wait();
            if(VERBOSE) cout << apps[i]->argv[0] << " exited with " << res << "\n";
        }

        cycles_t overall_end = Time::stop(0x1235);
        cycles_t end = Time::stop(0x1234);
        if(j >= warmup)
            res.push(end - start);
        cout << "Time: " << (end - start) << ", total: " << (overall_end - overall_start) << "\n";

        if(VERBOSE) cout << "Deleting VPEs...\n";

        for(size_t i = 0; i < instances * 2; ++i) {
            delete pipes[i / 2];
            pipes[i / 2] = nullptr;
            delete mems[i / 2];
            mems[i / 2] = nullptr;
            delete apps[i];
        }
    }

    OStringStream name;
    const char *s = wr_name;
    int underscores = 0;
    while(*s) {
        if(*s == '_') {
            if(++underscores == 2)
                break;
            name << '-';
        }
        else
            name << *s;
        s++;
    }
    WVPERF(name.str(), res);

    if(VERBOSE) cout << "Shutting down servers...\n";

    for(size_t i = 0; i < ARRAY_SIZE(srvs); ++i) {
        if(srvs[i])
            srvs[i]->request_shutdown();
    }

    for(size_t i = 0; i < ARRAY_SIZE(srvs); ++i) {
        if(!srv_vpes[i])
            continue;

        int res = srv_vpes[i]->wait();
        if(VERBOSE) cout << "server " << i << " exited with " << res << "\n";
        delete srvs[i];
        delete srv_vpes[i];
    }

    if(VERBOSE) cout << "Done\n";
    return 0;
}
