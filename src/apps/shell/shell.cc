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

#include <base/stream/IStringStream.h>
#include <base/time/Instant.h>

#include <m3/Syscalls.h>
#include <m3/accel/StreamAccel.h>
#include <m3/pipe/IndirectPipe.h>
#include <m3/session/VTerm.h>
#include <m3/stream/Standard.h>
#include <m3/tiles/ChildActivity.h>
#include <m3/tiles/Tile.h>
#include <m3/vfs/Dir.h>
#include <m3/vfs/VFS.h>

#include <algorithm>
#include <memory>
#include <stdlib.h>

#include "Args.h"
#include "Builtin.h"
#include "Input.h"
#include "Parser.h"
#include "Tokenizer.h"
#include "Vars.h"

using namespace m3;

static const CycleDuration ACOMP_TIME = CycleDuration::from_raw(4096);

static const size_t PIPE_SHM_SIZE = 512 * 1024;

static const uint MIN_EPS = 16;
static const TimeDuration MIN_TIME = TimeDuration::from_micros(100);
static const size_t MIN_PTS = 16;

static constexpr size_t MAX_CMDS = 8; // TODO get rid of this limit

static bool have_vterm = false;
static VTerm *vterm;

static std::unique_ptr<char *[]> build_args(const Parser::Command &cmd) {
    std::unique_ptr<char *[]> res(new char *[cmd.args()->size() + 1]);
    for(size_t i = 0; i < cmd.args()->size(); ++i)
        res[i] = (char *)expr_value(*cmd.args()->get(i));
    res[cmd.args()->size()] = nullptr;
    return res;
}

static const char *get_pe_name(const std::unique_ptr<Parser::VarList> &vars, const char *path) {
    FStream f(path, FILE_R | FILE_X);
    if(f.bad())
        return "";

    // accelerator description file?
    if(f.read() == '@' && f.read() == '=') {
        static char line[128];
        f.getline(line, sizeof(line));
        return line;
    }

    for(auto var = vars->cbegin(); var != vars->cend(); ++var) {
        if((*var)->name() == "TILE")
            return expr_value(*(*var)->value());
    }
    // prefer a different tile to prevent that we run out of EPs or similar
    return "core|own";
}

static void execute_pipeline(Pipes &pipesrv, std::unique_ptr<Parser::CmdList> &cmds) {
    bool builtin[MAX_CMDS];
    std::unique_ptr<IndirectPipe> pipes[MAX_CMDS] = {nullptr};
    std::unique_ptr<MemCap> mems[MAX_CMDS] = {nullptr};
    // destroy the activities first to prevent errors due to destroyed communication channels
    std::unique_ptr<StreamAccel> accels[MAX_CMDS] = {nullptr};
    Reference<Tile> tiles[MAX_CMDS];
    std::unique_ptr<ChildActivity> acts[MAX_CMDS] = {nullptr};

    // get tile types
    for(size_t i = 0; i < cmds->size(); ++i) {
        auto &cmd = cmds->get(i);
        if(cmd->args()->size() == 0) {
            eprintln("Command has no arguments"_cf);
            return;
        }

        const char *cmd_name = expr_value(*cmd->args()->get(0));
        builtin[i] = Builtin::is_builtin(cmd_name);
        if(i > 0 && builtin[i]) {
            eprintln("Builtin command cannot read from pipe"_cf);
            return;
        }
        if(!builtin[i]) {
            const char *tile_name = get_pe_name(cmd->vars(), cmd_name);
            tiles[i] = Tile::get(tile_name);
        }
    }

    size_t act_count = 0;
    FileRef<File> infile;
    FileRef<File> outfile;
    FileRef<File> errfile;
    for(size_t i = 0; i < cmds->size(); ++i) {
        auto &cmd = cmds->get(i);

        Vars vars;
        for(auto var = cmd->vars()->cbegin(); var != cmd->vars()->cend(); ++var)
            vars.set((*var)->name().c_str(), expr_value(*(*var)->value()));

        if(!builtin[i]) {
            // if we share our tile with this child activity, give it separate quotas to ensure
            // that we get our share (we don't trust the child apps)
            if(tiles[i]->sel() == Activity::own().tile()->sel()) {
                const auto [eps, time, pts] = tiles[i]->quota();
                if(eps.left > MIN_EPS && pts.left > MIN_PTS) {
                    tiles[i] = tiles[i]->derive(Some(eps.left - MIN_EPS),
                                                Some(time.total - MIN_TIME),
                                                Some(pts.left - MIN_PTS));
                }
                else
                    tiles[i] = Tile::get("core");
            }

            acts[i] = std::make_unique<ChildActivity>(tiles[i], expr_value(*cmd->args()->get(0)));
            act_count++;
        }

        // I/O redirection is only supported at the beginning and end
        if((i + 1 < cmds->size() && cmd->redirections()->std_out()) ||
           (i > 0 && cmd->redirections()->std_in())) {
            throw MessageException("Invalid I/O redirection");
        }

        fd_t infd = STDIN_FD;
        if(i == 0) {
            if(cmd->redirections()->std_in())
                infile =
                    VFS::open(expr_value(*cmd->redirections()->std_in()), FILE_R | FILE_NEWSESS);
            else if(vterm)
                infile = vterm->create_channel(true);
            if(infile.is_valid())
                infd = infile->fd();
        }
        else if((builtin[i - 1] || tiles[i - 1]->desc().is_programmable()) ||
                (builtin[i] || tiles[i]->desc().is_programmable()))
            infd = pipes[i - 1]->reader().fd();

        if(acts[i] && infd != STDIN_FD)
            acts[i]->add_file(STDIN_FD, infd);

        fd_t outfd = STDOUT_FD;
        if(i + 1 == cmds->size()) {
            if(cmd->redirections()->std_out())
                outfile = VFS::open(expr_value(*cmd->redirections()->std_out()),
                                    FILE_W | FILE_CREATE | FILE_TRUNC | FILE_NEWSESS);
            else if(vterm)
                outfile = vterm->create_channel(false);
            if(outfile.is_valid())
                outfd = outfile->fd();
        }
        else if((builtin[i] || tiles[i]->desc().is_programmable()) ||
                (builtin[i + 1] || tiles[i + 1]->desc().is_programmable())) {
            mems[i] = std::make_unique<MemCap>(MemCap::create_global(PIPE_SHM_SIZE, MemCap::RW));
            pipes[i] = std::make_unique<IndirectPipe>(pipesrv, *mems[i], PIPE_SHM_SIZE);
            outfd = pipes[i]->writer().fd();
        }

        if(acts[i] && outfd != STDOUT_FD)
            acts[i]->add_file(STDOUT_FD, outfd);

        std::unique_ptr<char *[]> args = build_args(*cmd);

        if(builtin[i]) {
            Builtin::execute(args.get(), outfd);
            // close stdout pipe to send EOF
            if(pipes[i])
                pipes[i]->close_writer();
        }
        else if(tiles[i]->desc().is_programmable()) {
            if(vterm)
                errfile = vterm->create_channel(false);
            if(errfile.is_valid())
                acts[i]->add_file(STDERR_FD, errfile->fd());

            acts[i]->add_mount("/", "/");

            acts[i]->exec(static_cast<int>(cmd->args()->size()),
                          const_cast<const char **>(args.get()), vars.get());
        }
        else
            accels[i] = std::make_unique<StreamAccel>(acts[i], ACOMP_TIME);

        if(i > 0 && pipes[i - 1]) {
            if(acts[i] && acts[i]->tile_desc().is_programmable())
                pipes[i - 1]->close_reader();
            if(acts[i - 1] && acts[i - 1]->tile_desc().is_programmable())
                pipes[i - 1]->close_writer();
        }
    }

    // connect input/output of accelerators
    if(act_count > 0) {
        FileRef<File> clones[act_count * 2];
        size_t c = 0;
        for(size_t i = 0; i < act_count; ++i) {
            if(accels[i]) {
                fd_t our_in_fd = acts[i]->get_file(STDIN_FD);
                if(our_in_fd != FileTable::MAX_FDS) {
                    auto our_in = Activity::own().files()->get(our_in_fd);
                    auto ain = our_in->clone();
                    accels[i]->connect_input(static_cast<GenericFile *>(&*ain));
                    clones[c++] = std::move(ain);
                }
                else if(accels[i - 1])
                    accels[i]->connect_input(accels[i - 1].get());

                fd_t our_out_fd = acts[i]->get_file(STDOUT_FD);
                if(our_out_fd != FileTable::MAX_FDS) {
                    auto our_out = Activity::own().files()->get(our_out_fd);
                    auto aout = our_out->clone();
                    accels[i]->connect_output(static_cast<GenericFile *>(&*aout));
                    clones[c++] = std::move(aout);
                }
                else if(accels[i + 1])
                    accels[i]->connect_output(accels[i + 1].get());
            }
        }

        // start accelerator activities
        for(size_t i = 0; i < act_count; ++i) {
            if(accels[i])
                acts[i]->start();
        }

        for(size_t rem = act_count; rem > 0;) {
            capsel_t sels[act_count];
            for(size_t x = 0, i = 0; i < act_count; ++i) {
                if(acts[i])
                    sels[x++] = acts[i]->sel();
            }

            Syscalls::activity_wait(sels, rem, 1);

            bool signal = false;
            capsel_t act = KIF::INV_SEL;
            int exitcode = 0;
            if(have_vterm) {
                // fetch the signal first to ensure we don't have one from last time
                cin.file()->fetch_signal();
            }

            while(true) {
                const TCU::Message *msg;
                if((msg = RecvGate::upcall().fetch())) {
                    GateIStream is(RecvGate::upcall(), msg);
                    auto upcall = reinterpret_cast<const KIF::Upcall::ActivityWait *>(msg->data);
                    act = upcall->act_sel;
                    exitcode = upcall->exitcode;
                    reply_vmsg(is, 0);
                    break;
                }
                else if(have_vterm && cin.file()->fetch_signal()) {
                    signal = true;
                    Syscalls::activity_wait(sels, 0, 1);
                    break;
                }

                OwnActivity::sleep();
            }

            for(size_t i = 0; i < act_count; ++i) {
                if(acts[i] && (signal || acts[i]->sel() == act)) {
                    if(exitcode != 0) {
                        eprintln("{} terminated with exit code {}"_cf,
                                 expr_value(*cmds->get(i)->args()->get(0)),
                                 static_cast<Errors::Code>(exitcode));
                    }
                    else if(signal) {
                        eprintln("{} terminated by signal"_cf,
                                 expr_value(*cmds->get(i)->args()->get(0)));
                    }
                    if(!acts[i]->tile_desc().is_programmable()) {
                        if(pipes[i])
                            pipes[i]->close_writer();
                        if(i > 0 && pipes[i - 1])
                            pipes[i - 1]->close_reader();
                    }
                    delete acts[i].release();
                    acts[i] = nullptr;
                    rem--;
                }
            }
        }
    }
}

static void execute(Pipes &pipesrv, std::unique_ptr<Parser::CmdList> &list) {
    // ignore empty commands
    if(list->size() == 0)
        return;

    for(auto it = list->cbegin(); it != list->cend(); ++it) {
        Args::prefix_path((*it)->args());
        Args::expand((*it)->args());
    }

    try {
        execute_pipeline(pipesrv, list);
    }
    catch(const Exception &e) {
        eprintln("command failed: {}"_cf, e.what());
    }
}

size_t prompt_len() {
    return strlen(VFS::cwd()) + 3;
}

void print_prompt() {
    print("{} $ "_cf, VFS::cwd());
}

int main(int argc, char **argv) {
    Pipes pipesrv("pipes");

    try {
        vterm = new VTerm("vterm");

        // change stdin, stdout, and stderr to vterm
        const fd_t fds[] = {STDIN_FD, STDOUT_FD, STDERR_FD};
        for(fd_t fd : fds)
            Activity::own().files()->set(fd, vterm->create_channel(fd == STDIN_FD));
        have_vterm = true;
    }
    catch(const Exception &e) {
        eprintln("Unable to open vterm: {}"_cf, e.what());
    }

    VFS::set_cwd("/");

    if(argc > 1) {
        OStringStream os;
        for(int i = 1; i < argc; ++i)
            format_to(os, "{} "_cf, argv[i]);

        try {
            Parser parser(Tokenizer::tokenize(os.str()));
            auto cmdlist = parser.parse();

            auto start = TimeInstant::now();
            execute(pipesrv, cmdlist);
            auto end = TimeInstant::now();

            println("Execution took {}"_cf, end.duration_since(start));
        }
        catch(const Exception &e) {
            eprintln("Unable to execute command: {}"_cf, e.what());
        }
        return 0;
    }

    println("========================"_cf);
    println("Welcome to the M3 shell!"_cf);
    println("========================"_cf);
    println();

    char buffer[256];
    while(!cin.eof()) {
        print_prompt();
        cout.flush();

        if(have_vterm)
            cin.file()->set_tmode(GenericFile::TMode::RAW);
        ssize_t len = Input::readline(buffer, sizeof(buffer));
        if(have_vterm)
            cin.file()->set_tmode(GenericFile::TMode::COOKED);
        if(len < 0)
            break;

        try {
            Parser parser(Tokenizer::tokenize(buffer));
            auto cmdlist = parser.parse();
            execute(pipesrv, cmdlist);
        }
        catch(const Exception &e) {
            eprintln("Unable to execute command: {}"_cf, e.what());
        }
    }
    return 0;
}
