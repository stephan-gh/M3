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

#include <base/log/Lib.h>
#include <base/stream/IStringStream.h>
#include <base/util/Time.h>

#include <m3/accel/StreamAccel.h>
#include <m3/stream/Standard.h>
#include <m3/pipe/IndirectPipe.h>
#include <m3/vfs/Dir.h>
#include <m3/vfs/VFS.h>
#include <m3/Syscalls.h>
#include <m3/VPE.h>

#include <memory>

#include "Args.h"
#include "Parser.h"
#include "Vars.h"

using namespace m3;

static const size_t ACOMP_TIME = 4096;

static const size_t PIPE_SHM_SIZE   = 512 * 1024;

static struct {
    const char *name;
    PEDesc pe;
} petypes[] = {
    /* COMP_IMEM */  {"imem",  PEDesc(PEType::COMP_IMEM, PEISA::NONE)},
    /* COMP_EMEM */  {"emem",  PEDesc(PEType::COMP_EMEM, PEISA::NONE)},
    /* MEM       */  {"mem",   PEDesc(PEType::MEM, PEISA::NONE)},
};

static struct {
    const char *name;
    PEISA isa;
} isas[] = {
    {"FFT",      PEISA::ACCEL_FFT},
    {"ROT13",    PEISA::ACCEL_ROT13},
};

static PEDesc get_pe_type(const char *name) {
    for(size_t i = 0; i < ARRAY_SIZE(petypes); ++i) {
        if(strcmp(name, petypes[i].name) == 0)
            return petypes[i].pe;
    }
    return VPE::self().pe();
}

static char **build_args(Command *cmd) {
    char **res = new char*[cmd->args->count + 1];
    for(size_t i = 0; i < cmd->args->count; ++i)
        res[i] = (char*)expr_value(cmd->args->args[i]);
    res[cmd->args->count] = nullptr;
    return res;
}

static PEDesc get_pedesc(const VarList &vars, const char *path) {
    FStream f(path, FILE_R | FILE_X);
    if(f.bad())
        return VPE::self().pe();

    // accelerator description file?
    if(f.read() == '@' && f.read() == '=') {
        char line[128];
        f.getline(line, sizeof(line));
        for(size_t i = 0; i < ARRAY_SIZE(isas); ++i) {
            if(strcmp(isas[i].name, line) == 0)
                return PEDesc(PEType::COMP_IMEM, isas[i].isa);
        }
    }

    for(size_t i = 0; i < vars.count; ++i) {
        if(strcmp(vars.vars[i].name, "PE") == 0) {
            PEDesc pe = get_pe_type(expr_value(vars.vars[i].value));
            // use the current ISA for comp-PEs
            // TODO we could let the user specify the ISA
            if(pe.type() != PEType::MEM)
                pe = PEDesc(pe.type(), VPE::self().pe().isa(), pe.mem_size());
            break;
        }
    }
    return VPE::self().pe();
}

static void execute_assignment(CmdList *list) {
    Command *cmd = list->cmds[0];

    for(size_t i = 0; i < cmd->vars->count; ++i) {
        Var *v = cmd->vars->vars + i;
        Vars::get().set(v->name, expr_value(v->value));
    }
}

static void execute_pipeline(Pipes &pipesrv, CmdList *list, bool muxed) {
    PEDesc descs[MAX_CMDS];
    std::unique_ptr<IndirectPipe> pipes[MAX_CMDS] = {nullptr};
    std::unique_ptr<MemGate> mems[MAX_CMDS] = {nullptr};
    // destroy the VPEs first to prevent errors due to destroyed communication channels
    std::unique_ptr<StreamAccel> accels[MAX_CMDS] = {nullptr};
    std::unique_ptr<VPE> vpes[MAX_CMDS] = {nullptr};

    // get PE types
    for(size_t i = 0; i < list->count; ++i) {
        if(list->cmds[i]->args->count == 0) {
            errmsg("Command has no arguments");
            return;
        }

        descs[i] = get_pedesc(*list->cmds[i]->vars, expr_value(list->cmds[i]->args->args[0]));
    }

    size_t vpe_count = 0;
    fd_t infd = STDIN_FD;
    fd_t outfd = STDOUT_FD;
    for(size_t i = 0; i < list->count; ++i) {
        Command *cmd = list->cmds[i];

        auto args = VPEArgs().pedesc(descs[i]).flags(muxed ? VPE::MUXABLE : 0);
        vpes[i] = std::make_unique<VPE>(expr_value(cmd->args->args[0]), args);
        vpe_count++;

        // I/O redirection is only supported at the beginning and end
        if((i + 1 < list->count && cmd->redirs->fds[STDOUT_FD]) ||
            (i > 0 && cmd->redirs->fds[STDIN_FD])) {
            throw MessageException("Invalid I/O redirection");
        }

        if(i == 0) {
            if(cmd->redirs->fds[STDIN_FD])
                infd = VFS::open(cmd->redirs->fds[STDIN_FD], FILE_R);
            vpes[i]->fds()->set(STDIN_FD, VPE::self().fds()->get(infd));
        }
        else if(descs[i - 1].is_programmable() || descs[i].is_programmable())
            vpes[i]->fds()->set(STDIN_FD, VPE::self().fds()->get(pipes[i - 1]->reader_fd()));

        if(i + 1 == list->count) {
            if(cmd->redirs->fds[STDOUT_FD])
                outfd = VFS::open(cmd->redirs->fds[STDOUT_FD], FILE_W | FILE_CREATE | FILE_TRUNC);
            vpes[i]->fds()->set(STDOUT_FD, VPE::self().fds()->get(outfd));
        }
        else if(descs[i].is_programmable() || descs[i + 1].is_programmable()) {
            mems[i] = std::make_unique<MemGate>(MemGate::create_global(PIPE_SHM_SIZE, MemGate::RW));
            pipes[i] = std::make_unique<IndirectPipe>(pipesrv, *mems[i], PIPE_SHM_SIZE);
            vpes[i]->fds()->set(STDOUT_FD, VPE::self().fds()->get(pipes[i]->writer_fd()));
        }

        if(descs[i].is_programmable()) {
            vpes[i]->fds()->set(STDERR_FD, VPE::self().fds()->get(STDERR_FD));
            vpes[i]->obtain_fds();

            vpes[i]->mounts(VPE::self().mounts());
            vpes[i]->obtain_mounts();

            char **args = build_args(cmd);
            vpes[i]->exec(static_cast<int>(cmd->args->count), const_cast<const char**>(args));
            delete[] args;
        }
        else
            accels[i] = std::make_unique<StreamAccel>(vpes[i], ACOMP_TIME);

        if(i > 0 && pipes[i - 1]) {
            if(vpes[i]->pe().is_programmable())
                pipes[i - 1]->close_reader();
            if(vpes[i - 1]->pe().is_programmable())
                pipes[i - 1]->close_writer();
        }
    }

    // connect input/output of accelerators
    {
        Reference<File> clones[vpe_count * 2];
        size_t c = 0;
        for(size_t i = 0; i < vpe_count; ++i) {
            if(accels[i]) {
                auto in = vpes[i]->fds()->get(STDIN_FD);
                if(in) {
                    auto ain = in.get() == VPE::self().fds()->get(STDIN_FD).get() ? in->clone() : in;
                    accels[i]->connect_input(static_cast<GenericFile*>(ain.get()));
                    if(ain.get() != in.get())
                        clones[c++] = ain;
                }
                else if(accels[i - 1])
                    accels[i]->connect_input(accels[i - 1].get());

                auto out = vpes[i]->fds()->get(STDOUT_FD);
                if(out) {
                    auto aout = out.get() == VPE::self().fds()->get(STDOUT_FD).get() ? out->clone() : out;
                    accels[i]->connect_output(static_cast<GenericFile*>(aout.get()));
                    if(aout.get() != out.get())
                        clones[c++] = aout;
                }
                else if(accels[i + 1])
                    accels[i]->connect_output(accels[i + 1].get());
            }
        }

        // start accelerator VPEs
        for(size_t i = 0; i < vpe_count; ++i) {
            if(accels[i])
                vpes[i]->start();
        }

        for(size_t rem = vpe_count; rem > 0; --rem) {
            capsel_t sels[vpe_count];
            for(size_t x = 0, i = 0; i < vpe_count; ++i) {
                if(vpes[i])
                    sels[x++] = vpes[i]->sel();
            }

            capsel_t vpe;
            int exitcode = Syscalls::vpe_wait(sels, rem, 0, &vpe);

            for(size_t i = 0; i < vpe_count; ++i) {
                if(vpes[i] && vpes[i]->sel() == vpe) {
                    if(exitcode != 0) {
                        cerr << expr_value(list->cmds[i]->args->args[0])
                             << " terminated with exit code " << exitcode << "\n";
                    }
                    if(!vpes[i]->pe().is_programmable()) {
                        if(pipes[i])
                            pipes[i]->close_writer();
                        if(i > 0 && pipes[i - 1])
                            pipes[i - 1]->close_reader();
                    }
                    delete vpes[i].release();
                    vpes[i] = nullptr;
                    break;
                }
            }
        }
    }
}

static void execute(Pipes &pipesrv, CmdList *list, bool muxed) {
    for(size_t i = 0; i < list->count; ++i) {
        Args::prefix_path(list->cmds[i]->args);
        Args::expand(list->cmds[i]->args);
    }

    try {
        if(list->count == 1 && list->cmds[0]->args->count == 0)
            execute_assignment(list);
        else
            execute_pipeline(pipesrv, list, muxed);
    }
    catch(const Exception &e) {
        errmsg("command failed: " << e.what());
    }
}

int main(int argc, char **argv) {
    Pipes pipesrv("pipes");

    bool muxed = argc > 1 && strcmp(argv[1], "1") == 0;

    if(argc > 2) {
        OStringStream os;
        for(int i = 2; i < argc; ++i)
            os << argv[i] << " ";

        IStringStream is(StringRef(os.str(), os.length()));
        CmdList *list = get_command(&is);
        if(!list)
            exitmsg("Unable to parse command '" << os.str() << "'");

        cycles_t start = Time::start(0x1234);
        execute(pipesrv, list, muxed);
        cycles_t end = Time::stop(0x1234);
        ast_cmds_destroy(list);

        cerr << "Execution took " << (end - start) << " cycles\n";
        return 0;
    }

    cout << "========================\n";
    cout << "Welcome to the M3 shell!\n";
    cout << "========================\n";
    cout << "\n";

    while(!cin.eof()) {
        cout << "$ ";
        cout.flush();

        CmdList *list = get_command(&cin);
        if(!list)
            continue;

        execute(pipesrv, list, muxed);
        ast_cmds_destroy(list);
    }
    return 0;
}
