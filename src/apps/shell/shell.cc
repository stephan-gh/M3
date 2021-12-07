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

#include <base/stream/IStringStream.h>
#include <base/util/Time.h>

#include <m3/accel/StreamAccel.h>
#include <m3/pipe/IndirectPipe.h>
#include <m3/session/VTerm.h>
#include <m3/stream/Standard.h>
#include <m3/vfs/Dir.h>
#include <m3/vfs/VFS.h>
#include <m3/Syscalls.h>
#include <m3/pes/PE.h>
#include <m3/pes/VPE.h>

#include <memory>

#include "Args.h"
#include "Input.h"
#include "Parser.h"
#include "Vars.h"

using namespace m3;

static const size_t ACOMP_TIME = 4096;

static const size_t PIPE_SHM_SIZE   = 512 * 1024;

static const uint MIN_EPS = 16;
static const uint64_t MIN_TIME = 100000; // 100µs
static const size_t MIN_PTS = 16;

static VTerm *vterm;
static RecvGate *signal_rgate;

static char **build_args(Command *cmd) {
    char **res = new char*[cmd->args->count + 1];
    for(size_t i = 0; i < cmd->args->count; ++i)
        res[i] = (char*)expr_value(cmd->args->args[i]);
    res[cmd->args->count] = nullptr;
    return res;
}

static String get_pe_name(size_t no, const VarList &vars, const char *path) {
    FStream f(path, FILE_R | FILE_X);
    if(f.bad())
        return "";

    // accelerator description file?
    if(f.read() == '@' && f.read() == '=') {
        static char line[128];
        f.getline(line, sizeof(line));
        return line;
    }

    for(size_t i = 0; i < vars.count; ++i) {
        if(strcmp(vars.vars[i].name, "PE") == 0)
            return expr_value(vars.vars[i].value);
    }
    // for the first program, prefer the same PE
    if(no == 0)
        return "own|core";
    // for the second, prefer another one
    return "core|own";
}

static void execute_assignment(CmdList *list) {
    Command *cmd = list->cmds[0];

    for(size_t i = 0; i < cmd->vars->count; ++i) {
        Var *v = cmd->vars->vars + i;
        Vars::get().set(v->name, expr_value(v->value));
    }
}

static void execute_pipeline(Pipes &pipesrv, CmdList *list) {
    String pe_names[MAX_CMDS];
    std::unique_ptr<IndirectPipe> pipes[MAX_CMDS] = {nullptr};
    std::unique_ptr<MemGate> mems[MAX_CMDS] = {nullptr};
    // destroy the VPEs first to prevent errors due to destroyed communication channels
    std::unique_ptr<StreamAccel> accels[MAX_CMDS] = {nullptr};
    Reference<PE> pes[MAX_CMDS];
    std::unique_ptr<VPE> vpes[MAX_CMDS] = {nullptr};

    // get PE types
    for(size_t i = 0; i < list->count; ++i) {
        if(list->cmds[i]->args->count == 0) {
            errmsg("Command has no arguments");
            return;
        }

        pe_names[i] = get_pe_name(i, *list->cmds[i]->vars, expr_value(list->cmds[i]->args->args[0]));
    }

    size_t vpe_count = 0;
    fd_t infd = -1;
    fd_t outfd = -1;
    for(size_t i = 0; i < list->count; ++i) {
        Command *cmd = list->cmds[i];

        pes[i] = PE::get(pe_names[i].c_str());
        // if we share our PE with this child VPE, give it separate quotas to ensure that we get our
        // share (we don't trust the child apps)
        if(pes[i]->sel() == VPE::self().pe()->sel()) {
            Quota<uint> eps;
            Quota<uint64_t> time;
            Quota<size_t> pts;
            pes[i]->quota(&eps, &time, &pts);
            if(eps.left > MIN_EPS && pts.left > MIN_PTS)
                pes[i] = pes[i]->derive(eps.left - MIN_EPS, time.total - MIN_TIME, pts.left - MIN_PTS);
            else
                pes[i] = PE::get("core");
        }

        vpes[i] = std::make_unique<VPE>(pes[i], expr_value(cmd->args->args[0]));
        vpe_count++;

        // I/O redirection is only supported at the beginning and end
        if((i + 1 < list->count && cmd->redirs->fds[STDOUT_FD]) ||
            (i > 0 && cmd->redirs->fds[STDIN_FD])) {
            throw MessageException("Invalid I/O redirection");
        }

        if(i == 0) {
            if(cmd->redirs->fds[STDIN_FD])
                infd = VFS::open(cmd->redirs->fds[STDIN_FD], FILE_R);
            else if(vterm)
                infd = VPE::self().fds()->alloc(vterm->create_channel(true));
            if(infd != -1)
                vpes[i]->fds()->set(STDIN_FD, VPE::self().fds()->get(infd));
        }
        else if(pes[i - 1]->desc().is_programmable() || pes[i]->desc().is_programmable())
            vpes[i]->fds()->set(STDIN_FD, VPE::self().fds()->get(pipes[i - 1]->reader_fd()));

        if(i + 1 == list->count) {
            if(cmd->redirs->fds[STDOUT_FD])
                outfd = VFS::open(cmd->redirs->fds[STDOUT_FD], FILE_W | FILE_CREATE | FILE_TRUNC);
            else if(vterm)
                outfd = VPE::self().fds()->alloc(vterm->create_channel(false));
            if(outfd != -1)
                vpes[i]->fds()->set(STDOUT_FD, VPE::self().fds()->get(outfd));
        }
        else if(pes[i]->desc().is_programmable() || pes[i + 1]->desc().is_programmable()) {
            mems[i] = std::make_unique<MemGate>(MemGate::create_global(PIPE_SHM_SIZE, MemGate::RW));
            pipes[i] = std::make_unique<IndirectPipe>(pipesrv, *mems[i], PIPE_SHM_SIZE);
            vpes[i]->fds()->set(STDOUT_FD, VPE::self().fds()->get(pipes[i]->writer_fd()));
        }

        if(pes[i]->desc().is_programmable()) {
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
            if(vpes[i]->pe_desc().is_programmable())
                pipes[i - 1]->close_reader();
            if(vpes[i - 1]->pe_desc().is_programmable())
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

        for(size_t rem = vpe_count; rem > 0; ) {
            capsel_t sels[vpe_count];
            for(size_t x = 0, i = 0; i < vpe_count; ++i) {
                if(vpes[i])
                    sels[x++] = vpes[i]->sel();
            }

            Syscalls::vpe_wait(sels, rem, 1, nullptr);

            bool signal = false;
            capsel_t vpe = KIF::INV_SEL;
            int exitcode = 0;

            while(true) {
                const TCU::Message *msg;
                if((msg = RecvGate::upcall().fetch())) {
                    GateIStream is(RecvGate::upcall(), msg);
                    auto upcall = reinterpret_cast<const KIF::Upcall::VPEWait*>(msg->data);
                    vpe = upcall->vpe_sel;
                    exitcode = upcall->exitcode;
                    reply_vmsg(is, 0);
                    break;
                }
                else if(signal_rgate && (msg = signal_rgate->fetch())) {
                    GateIStream is(*signal_rgate, msg);
                    signal = true;
                    reply_vmsg(is, 0);
                    Syscalls::vpe_wait(sels, 0, 1, nullptr);
                    break;
                }

                VPE::sleep();
            }

            for(size_t i = 0; i < vpe_count; ++i) {
                if(vpes[i] && (signal || vpes[i]->sel() == vpe)) {
                    if(exitcode != 0) {
                        cerr << expr_value(list->cmds[i]->args->args[0])
                             << " terminated with exit code " << exitcode << "\n";
                    }
                    else if(signal) {
                        cerr << expr_value(list->cmds[i]->args->args[0])
                             << " terminated by signal\n";
                    }
                    if(!vpes[i]->pe_desc().is_programmable()) {
                        if(pipes[i])
                            pipes[i]->close_writer();
                        if(i > 0 && pipes[i - 1])
                            pipes[i - 1]->close_reader();
                    }
                    delete vpes[i].release();
                    vpes[i] = nullptr;
                    rem--;
                }
            }
        }

        // close our input/output file; the server will recursively close all clones
        if(outfd != -1)
            VPE::self().fds()->remove(outfd);
        if(infd != -1)
            VPE::self().fds()->remove(infd);
    }
}

static void execute(Pipes &pipesrv, CmdList *list) {
    for(size_t i = 0; i < list->count; ++i) {
        Args::prefix_path(list->cmds[i]->args);
        Args::expand(list->cmds[i]->args);
    }

    try {
        if(list->count == 1 && list->cmds[0]->args->count == 0)
            execute_assignment(list);
        else
            execute_pipeline(pipesrv, list);
    }
    catch(const Exception &e) {
        errmsg("command failed: " << e.what());
    }
}

int main(int argc, char **argv) {
    Pipes pipesrv("pipes");

    bool have_vterm = false;
    try {
        vterm = new VTerm("vterm");

        // change stdin, stdout, and stderr to vterm
        const fd_t fds[] = {STDIN_FD, STDOUT_FD, STDERR_FD};
        for(fd_t fd : fds)
            VPE::self().fds()->set(fd, vterm->create_channel(fd == STDIN_FD));

        // register SendGate for signals from vterm
        signal_rgate = new RecvGate(RecvGate::create(5, 5));
        signal_rgate->activate();
        // create on the heap to keep it around
        SendGate *signal_sgate = new SendGate(SendGate::create(signal_rgate));
        cin.file()->set_signal_gate(*signal_sgate);

        have_vterm = true;
    }
    catch(const Exception &e) {
        errmsg("Unable to open vterm: " << e.what());
    }

    if(argc > 1) {
        OStringStream os;
        for(int i = 1; i < argc; ++i)
            os << argv[i] << " ";

        CmdList *list = parse_command(os.str());
        if(!list)
            exitmsg("Unable to parse command '" << os.str() << "'");

        cycles_t start = Time::start(0x1234);
        execute(pipesrv, list);
        cycles_t end = Time::stop(0x1234);
        ast_cmds_destroy(list);

        cerr << "Execution took " << (end - start) << " cycles\n";
        return 0;
    }

    cout << "========================\n";
    cout << "Welcome to the M3 shell!\n";
    cout << "========================\n";
    cout << "\n";

    char buffer[256];
    while(!cin.eof()) {
        cout << "$ ";
        cout.flush();

        if(have_vterm)
            cin.file()->set_tmode(GenericFile::TMode::RAW);
        ssize_t len = Input::readline(buffer, sizeof(buffer));
        if(have_vterm)
            cin.file()->set_tmode(GenericFile::TMode::COOKED);
        if(len < 0)
            break;

        CmdList *list = parse_command(buffer);
        if(!list)
            continue;

        execute(pipesrv, list);
        ast_cmds_destroy(list);
    }
    return 0;
}
