/*
 * Copyright (C) 2015-2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
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
#include <base/stream/IStringStream.h>
#include <base/util/Profile.h>
#include <base/Panic.h>
#include <base/CmdArgs.h>

#include <m3/session/LoadGen.h>
#include <m3/session/M3FS.h>
#include <m3/stream/Standard.h>
#include <m3/vfs/Dir.h>
#include <m3/vfs/VFS.h>
#include <m3/Test.h>

#include <vector>

#include "traceplayer.h"

using namespace m3;

static const size_t MAX_TMP_FILES   = 128;
static const bool VERBOSE           = 0;
static const uint META_EPS          = 4;

static m3::LoadGen::Channel *chan;

static void remove_rec(const char *path) {
    if(VERBOSE) cerr << "Unlinking " << path << "\n";

    try {
        VFS::unlink(path);
    }
    catch(const m3::Exception &e) {
        if(e.code() == Errors::IS_DIR) {
            Dir::Entry e;
            char tmp[128];
            Dir dir(path);
            while(dir.readdir(e)) {
                if(strcmp(e.name, ".") == 0 || strcmp(e.name, "..") == 0)
                    continue;

                OStringStream file(tmp, sizeof(tmp));
                file << path << "/" << e.name;
                remove_rec(file.str());
            }
            VFS::rmdir(path);
        }
    }
}

static void cleanup() {
    try {
        Dir dir("/tmp");

        std::vector<String> entries;

        if(VERBOSE) cerr << "Collecting files in /tmp\n";

        // remove all entries; we assume here that they are files
        Dir::Entry e;
        char path[128];
        while(dir.readdir(e)) {
            if(strcmp(e.name, ".") == 0 || strcmp(e.name, "..") == 0)
                continue;

            OStringStream file(path, sizeof(path));
            file << "/tmp/" << e.name;
            entries.push_back(file.str());
        }

        for(String &s : entries)
            remove_rec(s.c_str());
    }
    catch(...) {
        // ignore
    }
}

static void usage(const char *name) {
    cerr << "Usage: " << name << " [-p <prefix>] [-n <iterations>] [-w] [-f <fs>] [-t] [-v] [-u <warmup>]"
                              << " [-g <rgate selector>] [-l <loadgen>] [-i] [-d] <name>\n";
    exit(1);
}

int main(int argc, char **argv) {
    // defaults
    ulong iters         = 1;
    ulong warmup        = 0;
    bool keep_time      = false;
    bool stdio          = false;
    bool data           = false;
    bool wvtest         = false;
    bool verbose        = false;
    const char *fs      = "m3fs";
    const char *prefix  = "";
    const char *loadgen = "";
    capsel_t rgate      = ObjCap::INVALID;

    int opt;
    while((opt = CmdArgs::get(argc, argv, "p:n:wf:g:l:idtu:v")) != -1) {
        switch(opt) {
            case 'p': prefix = CmdArgs::arg; break;
            case 'n': iters = IStringStream::read_from<ulong>(CmdArgs::arg); break;
            case 'w': keep_time = true; break;
            case 'f': fs = CmdArgs::arg; break;
            case 'l': loadgen = CmdArgs::arg; break;
            case 'i': stdio = true; break;
            case 'd': data = true; break;
            case 't': wvtest = true; break;
            case 'u': warmup = IStringStream::read_from<ulong>(CmdArgs::arg); break;
            case 'v': verbose = true; break;
            case 'g': rgate = IStringStream::read_from<capsel_t>(CmdArgs::arg); break;
            default:
                usage(argv[0]);
        }
    }
    if(CmdArgs::ind >= argc)
        usage(argv[0]);

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
        try {
            VFS::mkdir(prefix, 0755);
        }
        catch(const m3::Exception &e) {
            if(e.code() != Errors::EXISTS)
                throw;
        }
    }

    TracePlayer player(prefix);

    Trace *trace = Traces::get(argv[CmdArgs::ind]);
    if(!trace)
        PANIC("Trace '" << argv[CmdArgs::ind] << "' does not exist.");

    // touch all operations to make sure we don't get pagefaults in trace_ops arrary
    unsigned int numTraceOps = 0;
    trace_op_t *op = trace->trace_ops;
    while (op && op->opcode != INVALID_OP) {
        if (op->opcode != WAITUNTIL_OP)
            numTraceOps++;
        op++;
    }

    if(rgate != ObjCap::INVALID) {
        RecvGate rg = RecvGate::bind(rgate, 6);
        {
            // tell the coordinator, that we are ready
            GateIStream msg = receive_msg(rg);
            reply_vmsg(msg, 1);
        }
        // wait until we should start
        receive_msg(rg);
    }

    // print parameters for reference
    cerr << "VPFS trace_bench started ["
         << "trace=" << argv[CmdArgs::ind] << ","
         << "n=" << iters << ","
         << "wait=" << (keep_time ? "yes" : "no") << ","
         << "data=" << (data ? "yes" : "no") << ","
         << "stdio=" << (stdio ? "yes" : "no") << ","
         << "prefix=" << prefix << ","
         << "fs=" << fs << ","
         << "loadgen=" << loadgen << ","
         << "ops=" << numTraceOps
         << "]\n";

    Profile pr(iters, warmup);
    struct FSTraceRunner : public Runner {
        std::function<void()> func;

        explicit FSTraceRunner(std::function<void()> func)
            : Runner(),
              func(func) {
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
            WVPERF(argv[CmdArgs::ind], pr.runner_with_id(runner, 0xFFFF));
        else
            pr.runner_with_id(runner, 0xFFFF);
    }
    catch (::Exception &e) {
        cerr << "Caught exception: " << e.msg() << "\n";
        return 1;
    }

    cerr << "VPFS trace_bench benchmark terminated\n";
    return 0;
}
