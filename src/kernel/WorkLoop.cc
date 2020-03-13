/*
 * Copyright (C) 2016-2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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
#include <base/log/Kernel.h>

#include <thread/ThreadManager.h>

#include "pes/PEMux.h"
#include "pes/VPEManager.h"
#include "SyscallHandler.h"
#include "WorkLoop.h"

#if defined(__host__)
#include <sys/types.h>
#include <sys/wait.h>
#include <sys/time.h>
#include <sys/stat.h>

static bool initialized = false;
static int sigchilds = 0;

static void sigchild(int) {
    sigchilds++;
    signal(SIGCHLD, sigchild);
}

static void kill_vpe(int pid, int status) {
    if(WIFEXITED(status)) {
        KLOG(VPES, "Child " << pid << " exited with status " << WEXITSTATUS(status));
    }
    else if(WIFSIGNALED(status)) {
        KLOG(VPES, "Child " << pid << " was killed by signal " << WTERMSIG(status));
        if(WCOREDUMP(status))
            KLOG(VPES, "Child " << pid << " core dumped");
    }

    if(WIFSIGNALED(status) || WEXITSTATUS(status) == 255) {
        kernel::VPE *vpe = kernel::VPEManager::get().vpe_by_pid(pid);
        if(vpe)
            vpe->stop_app(0, false);
    }
}

static void check_childs() {
    int status;
    pid_t pid;
    if(m3::TCU::get().receive_knotify(&pid, &status))
        kill_vpe(pid, status);

    for(; sigchilds > 0; sigchilds--) {
        pid = wait(&status);
        if(pid != -1)
            kill_vpe(pid, status);
    }
}
#endif

namespace kernel {

WorkLoop WorkLoop::_wl;

void WorkLoop::multithreaded(uint count) {
    for(uint i = 0; i < count; ++i)
        new m3::Thread(thread_startup, nullptr);
}

void WorkLoop::thread_startup(void *) {
    WorkLoop &wl = WorkLoop::get();
    wl.run();

    wl.thread_shutdown();
}

void WorkLoop::thread_shutdown() {
    m3::ThreadManager::get().stop();
    ::exit(0);
}

void WorkLoop::run() {
#if defined(__host__)
    if(!initialized) {
        signal(SIGCHLD, sigchild);
        initialized = true;
    }
#endif

    m3::TCU &tcu = m3::TCU::get();
    static_assert(TCU::SYSC_REP_COUNT == 2, "Wrong SYSC_REP_COUNT");
    epid_t sysep0 = SyscallHandler::ep(0);
    epid_t sysep1 = SyscallHandler::ep(1);
    epid_t srvep = TCU::SERV_REP;
    epid_t pexep = TCU::PEX_REP;
    const m3::TCU::Message *msg;
    while(_run) {
        m3::TCU::get().sleep();

        msg = tcu.fetch_msg(sysep0);
        if(msg) {
            // we know the subscriber here, so optimize that a bit
            VPE *vpe = reinterpret_cast<VPE*>(msg->label);
            SyscallHandler::handle_message(vpe, msg);
        }

        msg = tcu.fetch_msg(sysep1);
        if(msg) {
            VPE *vpe = reinterpret_cast<VPE*>(msg->label);
            SyscallHandler::handle_message(vpe, msg);
        }

        msg = tcu.fetch_msg(srvep);
        if(msg) {
            SendQueue *sq = reinterpret_cast<SendQueue*>(msg->label);
            sq->received_reply(srvep, msg);
        }

        msg = tcu.fetch_msg(pexep);
        if(msg) {
            PEMux *pemux = reinterpret_cast<PEMux*>(msg->label);
            pemux->handle_call(msg);
        }

        m3::ThreadManager::get().yield();

#if defined(__host__)
        check_childs();
#endif
    }
}

}
