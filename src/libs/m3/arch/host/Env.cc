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
#include <m3/tiles/Activity.h>

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
    Syscalls::activity_ctrl(KIF::SEL_ACT, KIF::Syscall::VCTRL_INIT, arg);
}

void Env::on_exit_func(int status, void *) {
    // don't use Syscalls here, because Syscalls::_sendgate might already have been destroyed
    MsgBuf req_buf;
    auto &req = req_buf.cast<KIF::Syscall::ActivityCtrl>();
    req.opcode = KIF::Syscall::ACT_CTRL;
    req.act_sel = KIF::SEL_ACT;
    req.op = static_cast<xfer_t>(KIF::Syscall::VCTRL_STOP);
    req.arg = static_cast<xfer_t>(status);
    TCU::get().send(env()->first_std_ep + TCU::SYSC_SEP_OFF, req_buf, 0, TCU::INVALID_EP);

    stop_tcu();
    // destroy the enviromment here, because on_exit functions are called last
    delete _inst;
    _inst = nullptr;
}

static void load_params(Env *e) {
    char path[64];
    snprintf(path, sizeof(path), "%s/%d", Env::tmp_dir(), getpid());
    std::ifstream in(path);
    if(!in.good())
        PANIC("Unable to read " << path);

    tileid_t tile;
    epid_t ep;
    word_t credits;
    capsel_t first_sel;
    capsel_t kmem_sel;
    label_t lbl;
    std::string shm_prefix;
    in >> shm_prefix >> tile >> first_sel >> kmem_sel >> lbl >> ep >> credits;

    e->set_params(tile, shm_prefix, lbl, ep, credits, first_sel, kmem_sel);
}

WEAK void Env::init() {
    std::set_terminate(Exception::terminate_handler);

    char log_file[256];
    snprintf(log_file, sizeof(log_file), "%s/log.txt", Env::out_dir());
    int logfd = open(log_file, O_WRONLY | O_APPEND);

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
    uintptr_t addr = 0;
    TCU::get().configure_recv(TCU::SYSC_REP_OFF, addr, SYSC_RBUF_ORDER, SYSC_RBUF_ORDER);
    addr += SYSC_RBUF_SIZE;

    TCU::get().configure_recv(TCU::UPCALL_REP_OFF, addr, UPCALL_RBUF_ORDER, UPCALL_RBUF_ORDER);
    addr += UPCALL_RBUF_SIZE;

    TCU::get().configure_recv(TCU::DEF_REP_OFF, addr, DEF_RBUF_ORDER, DEF_RBUF_ORDER);

    TCU::get().configure(TCU::SYSC_SEP_OFF, _sysc_label, 0, 0, _sysc_epid,
        _sysc_credits, SYSC_RBUF_ORDER);

    TCU::get().start();
}

}
