/**
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

#include <m3/server/RemoteServer.h>
#include <m3/stream/Standard.h>
#include <m3/pipe/IndirectPipe.h>
#include <m3/vfs/VFS.h>
#include <m3/VPE.h>

using namespace m3;

static constexpr bool VERBOSE           = true;
static constexpr size_t PIPE_SHM_SIZE   = 512 * 1024;

enum Mode {
    DEDICATED,
    SERV_MUXED,
    ALL_MUXED,
};

enum Memory {
    DRAM,
    SPM,
};

struct App {
    explicit App(const char *name, const char *pager, bool muxed)
        : name(name),
          vpe(name, VPEArgs().pager(pager).flags(muxed ? VPE::MUXABLE : 0)) {
    }

    const char *name;
    VPE vpe;
};

static App *create(const char *name, const char *pager, bool muxable) {
    if(VERBOSE) cout << "VPE: " << name << "\n";
    return new App(name, pager, muxable);
}

static void usage(const char *name) {
    cerr << "Usage: " << name << " [-m <mode>] [-p <pipe-mem>] [-r <repeats>] <wargs> <rargs> ...\n";
    cerr << "  <mode> can be:\n";
    cerr << "    'ded':      all use dedicated PEs\n";
    cerr << "    'serv-mux': services share a PE\n";
    cerr << "    'all-mux':  all share the PEs\n";
    cerr << "  <pipe-mem> can be:\n";
    cerr << "    'dram':     put pipe's shared memory in DRAM\n";
    cerr << "    'spm':      put pipe's shared memory in neighboring SPM\n";
    cerr << "  <repeats> specifies the number of repetitions of the benchmark\n";
    exit(1);
}

int main(int argc, char **argv) {
    Mode mode = DEDICATED;
    Memory pmem = DRAM;
    int repeats = 1;

    int opt;
    while((opt = CmdArgs::get(argc, argv, "m:p:r:")) != -1) {
        switch(opt) {
            case 'm': {
                if(strcmp(CmdArgs::arg, "ded") == 0)
                    mode = Mode::DEDICATED;
                else if(strcmp(CmdArgs::arg, "serv-mux") == 0)
                    mode = Mode::SERV_MUXED;
                else if(strcmp(CmdArgs::arg, "all-mux") == 0)
                    mode = Mode::ALL_MUXED;
                else
                    usage(argv[0]);
                break;
            }
            case 'p': {
                if(strcmp(CmdArgs::arg, "dram") == 0)
                    pmem = Memory::DRAM;
                else if(strcmp(CmdArgs::arg, "spm") == 0)
                    pmem = Memory::SPM;
                else
                    usage(argv[0]);
                break;
            }
            case 'r': repeats = IStringStream::read_from<int>(CmdArgs::arg); break;
            default:
                usage(argv[0]);
        }
    }
    if(CmdArgs::ind + 1 >= argc)
        usage(argv[0]);

    int wargs = IStringStream::read_from<int>(argv[CmdArgs::ind + 0]);
    int rargs = IStringStream::read_from<int>(argv[CmdArgs::ind + 1]);

    if(argc != CmdArgs::ind + 2 + wargs + rargs)
        usage(argv[0]);

    MemGate pipemem = MemGate::create_global(PIPE_SHM_SIZE, MemGate::RW);

    for(int j = 0; j < repeats; ++j) {
        App *apps[5] = {nullptr};
        RemoteServer *pagr_srv = nullptr;

        if(VERBOSE) cout << "Creating VPEs...\n";

#if defined(__gem5__)
        // start pager
        apps[2] = create("mypg", nullptr, mode == SERV_MUXED || mode == ALL_MUXED);
        pagr_srv = new RemoteServer(apps[2]->vpe, "mypg");

        {
            String pgarg = pagr_srv->sel_arg();
            const char *pager_args[] = {"/bin/pager", "-a", "16", "-f", "16", "-s", pgarg.c_str()};
            apps[2]->vpe.exec(ARRAY_SIZE(pager_args), pager_args);
        }
#endif

        char **wargv = argv + CmdArgs::ind + 2;
        char **rargv = argv + CmdArgs::ind + 2 + wargs;
        apps[3] = create(wargv[0], "mypg", mode == ALL_MUXED);
        apps[4] = create(rargv[0], "mypg", mode == ALL_MUXED);
        apps[0] = create("pipes", nullptr, mode == SERV_MUXED || mode == ALL_MUXED);
        apps[1] = create("m3fs", nullptr, mode == SERV_MUXED || mode == ALL_MUXED);

        RemoteServer *m3fs_srv = nullptr;
        RemoteServer *pipe_srv = new RemoteServer(apps[0]->vpe, "mypipes");
        if(apps[1])
            m3fs_srv = new RemoteServer(apps[1]->vpe, "mym3fs");

        if(VERBOSE) cout << "Starting services...\n";

        // start services
        String pipearg = pipe_srv->sel_arg();
        const char *pipe_args[] = {"/bin/pipes", "-s", pipearg.c_str()};
        apps[0]->vpe.exec(ARRAY_SIZE(pipe_args), pipe_args);

        if(apps[1]) {
            String m3fsarg = m3fs_srv->sel_arg();
            const char *m3fs_args[] = {"/bin/m3fs", "-s", m3fsarg.c_str(), "mem", "268435456"};
            apps[1]->vpe.exec(ARRAY_SIZE(m3fs_args), m3fs_args);
        }

        {
            Pipes pipes("mypipes");

            // create pipe
            std::unique_ptr<MemGate> vpemem;
            std::unique_ptr<VPE> memvpe;
            std::unique_ptr<IndirectPipe> pipe;
            if(pmem == DRAM)
                pipe.reset(new IndirectPipe(pipes, pipemem, PIPE_SHM_SIZE));
            else {
                memvpe.reset(new VPE("mem"));
                vpemem.reset(new MemGate(memvpe->mem().derive(0x10000, PIPE_SHM_SIZE, MemGate::RW)));
                pipe.reset(new IndirectPipe(pipes, *vpemem, PIPE_SHM_SIZE));
                // let the kernel schedule the VPE; this cannot be done by the reader/writer, because
                // the pipe service just configures their EP, but doesn't delegate the memory capability
                // to them
                vpemem->write(&vpemem, sizeof(vpemem), 0);
            }

            if(VERBOSE) cout << "Starting reader and writer...\n";

            if(apps[1])
                VFS::mount("/foo", "m3fs", "mym3fs");

            cycles_t start = Time::start(0x1234);

            // start writer
            apps[3]->vpe.fds()->set(STDOUT_FD, VPE::self().fds()->get(pipe->writer_fd()));
            apps[3]->vpe.obtain_fds();
            apps[3]->vpe.mounts(VPE::self().mounts());
            apps[3]->vpe.obtain_mounts();
            apps[3]->vpe.exec(wargs, const_cast<const char**>(wargv));

            // start reader
            apps[4]->vpe.fds()->set(STDIN_FD, VPE::self().fds()->get(pipe->reader_fd()));
            apps[4]->vpe.obtain_fds();
            apps[4]->vpe.mounts(VPE::self().mounts());
            apps[4]->vpe.obtain_mounts();
            apps[4]->vpe.exec(rargs, const_cast<const char**>(rargv));

            pipe->close_writer();
            pipe->close_reader();

            if(VERBOSE) cout << "Waiting for applications...\n";
            cycles_t runstart = Time::start(0x1111);

            // don't wait for the services
            for(size_t i = 3; i < ARRAY_SIZE(apps); ++i) {
                int res = apps[i]->vpe.wait();
                if(VERBOSE) cout << apps[i]->name << " exited with " << res << "\n";
            }

            cycles_t runend = Time::stop(0x1111);
            cycles_t end = Time::stop(0x1234);
            cout << "Time: " << (end - start) << ", runtime: " << (runend - runstart) << "\n";

            if(VERBOSE) cout << "Waiting for services...\n";

            // destroy pipe first
        }

        if(apps[1])
            VFS::unmount("/foo");

        // request shutdown
        pipe_srv->request_shutdown();
        if(pagr_srv)
            pagr_srv->request_shutdown();
        if(m3fs_srv)
            m3fs_srv->request_shutdown();

        // wait for services
        for(size_t i = 0; i < 3; ++i) {
            if(!apps[i])
                continue;
            int res = apps[i]->vpe.wait();
            if(VERBOSE) cout << apps[i]->name << " exited with " << res << "\n";
        }

        if(VERBOSE) cout << "Deleting VPEs...\n";

        for(size_t i = 3; i < ARRAY_SIZE(apps); ++i)
            delete apps[i];

        delete m3fs_srv;
        delete pipe_srv;
        delete pagr_srv;

        for(size_t i = 0; i < 3; ++i)
            delete apps[i];

        if(VERBOSE) cout << "Done\n";
    }
    return 0;
}
