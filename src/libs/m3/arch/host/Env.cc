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

#include <base/log/Lib.h>
#include <base/Backtrace.h>
#include <base/Env.h>
#include <base/TCU.h>
#include <base/Init.h>
#include <base/Panic.h>

#include <m3/com/RecvGate.h>
#include <m3/stream/Standard.h>
#include <m3/Syscalls.h>
#include <m3/WorkLoop.h>
#include <m3/pes/VPE.h>

#include <sys/mman.h>
#include <fstream>
#include <unistd.h>
#include <fcntl.h>

namespace m3 {

struct PostInit {
    PostInit();
};

INIT_PRIO_ENV_POST PostInit postInit;

static void stop_tcu() {
    TCU::get().stop();
    pthread_join(TCU::get().tid(), nullptr);
}

static void init_syscall() {
    word_t arg = Env::eps_start();
    Syscalls::vpe_ctrl(KIF::SEL_VPE, KIF::Syscall::VCTRL_INIT, arg);
}

void Env::on_exit_func(int status, void *) {
    Syscalls::vpe_ctrl(KIF::SEL_VPE, KIF::Syscall::VCTRL_STOP, static_cast<xfer_t>(status));
    stop_tcu();
    // destroy the enviromment here, because on_exit functions are called last
    delete _inst;
    _inst = nullptr;
}

static void load_params(Env *e) {
    char path[64];
    snprintf(path, sizeof(path), "/tmp/m3/%d", getpid());
    std::ifstream in(path);
    if(!in.good())
        PANIC("Unable to read " << path);

    peid_t pe;
    epid_t ep;
    word_t credits;
    capsel_t first_sel;
    capsel_t kmem_sel;
    label_t lbl;
    std::string shm_prefix;
    in >> shm_prefix >> pe >> first_sel >> kmem_sel >> lbl >> ep >> credits;

    e->set_params(pe, shm_prefix, lbl, ep, credits, first_sel, kmem_sel);
}

EXTERN_C WEAK void init_env() {
    std::set_terminate(Exception::terminate_handler);

    int logfd = open("run/log.txt", O_WRONLY | O_APPEND);

    new Env(new HostEnvBackend(), logfd);
    load_params(env());

    // use on_exit to get the return-value of main and pass it to the m3 kernel
    on_exit(Env::on_exit_func, nullptr);
}

PostInit::PostInit() {
    env()->init_tcu();
    init_syscall();
}

void Env::init_tcu() {
    RecvGate &sysc = RecvGate::syscall();
    TCU::get().configure_recv(TCU::SYSC_REP, reinterpret_cast<uintptr_t>(sysc.addr()),
        SYSC_RBUF_ORDER, SYSC_RBUF_ORDER);

    RecvGate &upc = RecvGate::upcall();
    TCU::get().configure_recv(TCU::UPCALL_REP, reinterpret_cast<uintptr_t>(upc.addr()),
        UPCALL_RBUF_ORDER, UPCALL_RBUF_ORDER);

    RecvGate &def = RecvGate::upcall();
    TCU::get().configure_recv(TCU::DEF_REP, reinterpret_cast<uintptr_t>(def.addr()),
        DEF_RBUF_ORDER, DEF_RBUF_ORDER);

    TCU::get().configure(TCU::SYSC_SEP, _sysc_label, 0, 0, _sysc_epid, _sysc_credits, SYSC_RBUF_ORDER);

    TCU::get().start();
}

void Env::reset() {
    load_params(this);

    Serial::init(executable(), env()->pe);

    TCU::get().reset();

    init_tcu();

    // we have to call init for this VPE in case we hadn't done that yet
    init_syscall();
}

}
