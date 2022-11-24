/*
 * Copyright (C) 2015-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2022 Nils Asmussen, Barkhausen Institut
 *
 * This file is part of M3 (Microkernel-based SysteM for Heterogeneous Manycores).
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
#include <base/time/Profile.h>

#include <m3/Exception.h>
#include <m3/Test.h>
#include <m3/session/LoadGen.h>
#include <m3/session/M3FS.h>
#include <m3/stream/Standard.h>
#include <m3/vfs/Dir.h>
#include <m3/vfs/VFS.h>

#include <stdlib.h>
#include <unistd.h>
#include <vector>

#include "traceplayer.h"

using namespace m3;

static const size_t MAX_TMP_FILES = 128;
static const bool VERBOSE = 0;

static m3::LoadGen::Channel *chan;

static void remove_rec(const char *path) {
    if(VERBOSE)
        eprintln("Unlinking {}"_cf, path);

    if(VFS::try_unlink(path) == Errors::IS_DIR) {
        Dir::Entry e;
        char tmp[128];
        Dir dir(path);
        while(dir.readdir(e)) {
            if(strcmp(e.name, ".") == 0 || strcmp(e.name, "..") == 0)
                continue;

            OStringStream file(tmp, sizeof(tmp));
            format_to(file, "{}/{}"_cf, path, e.name);
            remove_rec(file.str());
        }
        VFS::rmdir(path);
    }
}

static void cleanup() {
    try {
        Dir dir("/tmp");

        std::vector<std::string> entries;

        if(VERBOSE)
            eprintln("Collecting files in /tmp"_cf);

        // remove all entries; we assume here that they are files
        Dir::Entry e;
        char path[128];
        while(dir.readdir(e)) {
            if(strcmp(e.name, ".") == 0 || strcmp(e.name, "..") == 0)
                continue;

            OStringStream file(path, sizeof(path));
            format_to(file, "/tmp/{}"_cf, e.name);
            entries.push_back(file.str());
        }

        for(std::string &s : entries)
            remove_rec(s.c_str());
    }
    catch(...) {
        // ignore
    }
}

static void usage(const char *name) {
    eprint("Usage: {} [-p <prefix>] [-n <iterations>] [-w] [-t] [-v] [-u <warmup>]"_cf, name);
    eprint(" [-g <rgate selector>] [-l <loadgen>] [-i] [-d]"_cf);
    eprintln(" [-f <mount_fs>] <name>"_cf);
    exit(1);
}

int main(int argc, char **argv) {
    // defaults
    ulong iters = 1;
    ulong warmup = 0;
    bool keep_time = false;
    bool stdio = false;
    bool data = false;
    bool wvtest = false;
    bool verbose = false;
    const char *prefix = "";
    const char *loadgen = "";
    const char *mount_fs = "";
    capsel_t rgate = ObjCap::INVALID;

    int opt;
    while((opt = getopt(argc, argv, "p:n:wg:l:idtu:f:v")) != -1) {
        switch(opt) {
            case 'p': prefix = optarg; break;
            case 'n': iters = IStringStream::read_from<ulong>(optarg); break;
            case 'w': keep_time = true; break;
            case 'l': loadgen = optarg; break;
            case 'i': stdio = true; break;
            case 'd': data = true; break;
            case 't': wvtest = true; break;
            case 'u': warmup = IStringStream::read_from<ulong>(optarg); break;
            case 'v': verbose = true; break;
            case 'g': rgate = IStringStream::read_from<capsel_t>(optarg); break;
            case 'f': mount_fs = optarg; break;
            default: usage(argv[0]);
        }
    }
    if(optind >= argc)
        usage(argv[0]);

    // mount fs, if required
    if(*mount_fs)
        VFS::mount("/", "m3fs", mount_fs);

    // connect to load generator
    if(*loadgen) {
        try {
            m3::LoadGen *lg = new m3::LoadGen(loadgen);
            chan = lg->create_channel(2 * 1024 * 1024);
            lg->start(3 * 11);
        }
        catch(...) {
            // ignore
        }
    }

    if(*prefix) {
        Errors::Code res = VFS::try_mkdir(prefix, 0755);
        if(res != Errors::SUCCESS && res != Errors::EXISTS)
            vthrow(res, "Unable to create directory {}"_cf, prefix);
    }

    TracePlayer player(prefix);

    Trace *trace = Traces::get(argv[optind]);
    if(!trace)
        exitmsg("Trace '{}' does not exist."_cf, argv[optind]);

    // touch all operations to make sure we don't get pagefaults in trace_ops arrary
    unsigned int numTraceOps = 0;
    trace_op_t *op = trace->trace_ops;
    while(op && op->opcode != INVALID_OP) {
        if(op->opcode != WAITUNTIL_OP)
            numTraceOps++;
        op++;
    }

    if(rgate != ObjCap::INVALID) {
        RecvGate rg = RecvGate::bind(rgate);
        {
            // tell the coordinator, that we are ready
            GateIStream msg = receive_msg(rg);
            reply_vmsg(msg, 1);
        }
        // wait until we should start
        receive_msg(rg);
    }

    // print parameters for reference
    eprintln(
        "VPFS trace_bench started [trace={}, n={}, wait={}, data={}, stdio={}, prefix={}, loadgen={}, ops={}]"_cf,
        argv[optind], iters, keep_time ? "yes" : "no", data ? "yes" : "no", stdio ? "yes" : "no",
        prefix, loadgen, numTraceOps);

    Profile pr(iters, warmup);
    struct FSTraceRunner : public Runner {
        std::function<void()> func;

        explicit FSTraceRunner(std::function<void()> func) : Runner(), func(func) {
        }

        void run() override {
            func();
        }
        void post() override {
            cleanup();
        }
    };

    try {
        FSTraceRunner runner([&] {
            player.play(trace, chan, data, stdio, keep_time, verbose);
        });
        if(wvtest)
            WVPERF(argv[optind], pr.runner<CycleInstant>(runner));
        else
            pr.runner<CycleInstant>(runner);
    }
    catch(::Exception &e) {
        eprintln("Caught exception: {}"_cf, e.msg());
        return 1;
    }

    eprintln("VPFS trace_bench benchmark terminated"_cf);
    return 0;
}
