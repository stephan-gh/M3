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
#include <base/stream/IStringStream.h>
#include <base/time/Profile.h>
#include <base/CmdArgs.h>
#include <base/Panic.h>

#include <m3/stream/Standard.h>
#include <m3/pipe/IndirectPipe.h>
#include <m3/tiles/ChildActivity.h>
#include <m3/vfs/Dir.h>
#include <m3/vfs/VFS.h>
#include <m3/Syscalls.h>
#include <m3/Test.h>

using namespace m3;

static constexpr bool VERBOSE = true;

struct App {
    explicit App(int argc, const char **argv)
        : argc(argc),
          argv(argv),
          tile(Tile::get("core")),
          act(tile, argv[0]),
          rgate(RecvGate::create(6, 6)),
          sgate(SendGate::create(&rgate)) {
        act.delegate_obj(rgate.sel());
    }

    int argc;
    const char **argv;
    Reference<Tile> tile;
    ChildActivity act;
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
    Reference<Tile> srv_tiles[2];
    Pipes pipesrv("pipes");

    if(VERBOSE) cout << "Creating application activities...\n";

    Results<CycleDuration> res(static_cast<ulong>(repeats));

    int exitcode = 0;
    for(int j = 0; j < warmup + repeats; ++j) {
        const size_t ARG_COUNT = 9;
        for(size_t i = 0; i < instances * 2; ++i) {
            const char **args = new const char *[ARG_COUNT];
            args[0] = "/bin/fstrace-m3fs-pipe";

            apps[i] = new App(ARG_COUNT, args);
        }

        if(VERBOSE) cout << "Starting activities...\n";

        auto overall_start = CycleInstant::now();

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
            args[6] = "-g";

            OStringStream rgatesel(new char[11], 11);
            rgatesel << apps[i]->rgate.sel();
            args[7] = rgatesel.str();
            args[8] = (i % 2 == 0) ? wr_name : rd_name;

            if(VERBOSE) {
                cout << "Starting ";
                for(size_t x = 0; x < ARG_COUNT; ++x)
                    cout << args[x] << " ";
                cout << "\n";
            }

            if(i % 2 == 0) {
                mems[i / 2] = new MemGate(MemGate::create_global(PIPE_SHM_SIZE, MemGate::RW));
                pipes[i / 2] = new IndirectPipe(pipesrv, *mems[i / 2], PIPE_SHM_SIZE, data ? 0 : FILE_NODATA);
                apps[i]->act.add_file(STDOUT_FD, pipes[i / 2]->writer_fd());
            }
            else
                apps[i]->act.add_file(STDIN_FD, pipes[i / 2]->reader_fd());

            apps[i]->act.add_mount("/", "/");

            apps[i]->act.exec(apps[i]->argc, apps[i]->argv);

            if(i % 2 == 1) {
                pipes[i / 2]->close_writer();
                pipes[i / 2]->close_reader();
            }
        }

        if(VERBOSE) cout << "Signaling activities...\n";

        for(size_t i = 0; i < instances * 2; ++i)
            send_receive_vmsg(apps[i]->sgate, 1);

        auto start = CycleInstant::now();

        for(size_t i = 0; i < instances * 2; ++i)
            send_vmsg(apps[i]->sgate, 1);

        if(VERBOSE) cout << "Waiting for activities...\n";

        for(size_t i = 0; i < instances * 2; ++i) {
            int res = apps[i]->act.wait();
            if(res != 0)
                exitcode = 1;
            if(VERBOSE) cout << apps[i]->argv[0] << " exited with " << res << "\n";
        }

        auto overall_end = CycleInstant::now();
        auto end = CycleInstant::now();
        if(j >= warmup)
            res.push(end.duration_since(start));
        cout << "Time: " << end.duration_since(start)
             << ", total: " << overall_end.duration_since(overall_start) << "\n";

        if(VERBOSE) cout << "Deleting activities...\n";

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

    if(VERBOSE) cout << "Done\n";
    return exitcode;
}
