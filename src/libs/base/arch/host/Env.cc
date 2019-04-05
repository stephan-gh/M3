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
#include <base/DTU.h>
#include <base/Init.h>
#include <base/Panic.h>

#include <m3/com/RecvGate.h>
#include <m3/Syscalls.h>
#include <m3/WorkLoop.h>
#include <m3/VPE.h>

#include <sys/mman.h>
#include <fstream>
#include <unistd.h>
#include <fcntl.h>

#ifndef NDEBUG
volatile int wait_for_debugger = 1;
#endif

namespace m3 {

void *Env::_mem = nullptr;
Env *Env::_inst = nullptr;
INIT_PRIO_ENV Env::Init Env::_init;
INIT_PRIO_ENV_POST Env::PostInit Env::_postInit;
char Env::_exec[128];
char Env::_exec_short[128];
const char *Env::_exec_short_ptr = nullptr;

static void stop_dtu() {
    DTU::get().stop();
    pthread_join(DTU::get().tid(), nullptr);
}

static void init_syscall() {
    word_t arg = Env::eps_start();
    Syscalls::get().vpectrl(VPE::self().sel(), KIF::Syscall::VCTRL_INIT, arg);
}

void Env::on_exit_func(int status, void *) {
    Syscalls::get().exit(status);
    stop_dtu();
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
    label_t lbl;
    std::string shm_prefix;
    in >> shm_prefix >> pe >> first_sel >> lbl >> ep >> credits;

    e->set_params(pe, shm_prefix, lbl, ep, credits, first_sel);
}

EXTERN_C WEAK void init_env() {
    int logfd = open("run/log.txt", O_WRONLY | O_APPEND);

    new Env(new HostEnvBackend(), logfd);
    load_params(env());

    // use on_exit to get the return-value of main and pass it to the m3 kernel
    on_exit(Env::on_exit_func, nullptr);
}

HostEnvBackend::HostEnvBackend() {
}

HostEnvBackend::~HostEnvBackend() {
}

Env::Init::Init() {
#ifndef NDEBUG
    char *val = getenv("M3_WAIT");
    if(val) {
        const char *exec = Env::executable();
        size_t vlen = strlen(val);
        size_t elen = strlen(exec);
        if(elen >= vlen && strcmp(exec + elen - vlen, val) == 0 &&
            (elen == vlen || exec[elen - vlen - 1] == '/')) {
            while(wait_for_debugger)
                usleep(20000);
        }
    }
#endif

    m3::Heap::init();
    init_env();

    Serial::init(executable(), env()->pe);
}

Env::Init::~Init() {
}

Env::PostInit::PostInit() {
    _inst->init_dtu();
    if(!env()->is_kernel())
        init_syscall();
}

void Env::reset() {
    load_params(this);

    Serial::init(executable(), env()->pe);

    DTU::get().reset();
    EPMux::get().reset();

    init_dtu();

    // we have to call init for this VPE in case we hadn't done that yet
    init_syscall();
}

Env::Env(EnvBackend *backend, int logfd)
    : pe(set_inst(this)),
      _backend(backend),
      _logfd(logfd),
      _shm_prefix(),
      _sysc_label(),
      _sysc_epid(),
      _sysc_credits(),
      _log_mutex(PTHREAD_MUTEX_INITIALIZER) {
}

void Env::init_executable() {
    int fd = open("/proc/self/cmdline", O_RDONLY);
    if(fd == -1)
        PANIC("open");
    if(read(fd, _exec, sizeof(_exec)) == -1)
        PANIC("read");
    close(fd);
    strncpy(_exec_short, _exec, sizeof(_exec_short));
    _exec_short[sizeof(_exec_short) - 1] = '\0';
    _exec_short_ptr = basename(_exec_short);
}

void Env::init_dtu() {
    RecvGate &sysc = RecvGate::syscall();
    DTU::get().configure_recv(DTU::SYSC_REP, reinterpret_cast<uintptr_t>(sysc.addr()),
        SYSC_RBUF_ORDER, SYSC_RBUF_ORDER);

    RecvGate &upc = RecvGate::upcall();
    DTU::get().configure_recv(DTU::UPCALL_REP, reinterpret_cast<uintptr_t>(upc.addr()),
        UPCALL_RBUF_ORDER, UPCALL_RBUF_ORDER);

    RecvGate &def = RecvGate::upcall();
    DTU::get().configure_recv(DTU::DEF_REP, reinterpret_cast<uintptr_t>(def.addr()),
        DEF_RBUF_ORDER, DEF_RBUF_ORDER);

    DTU::get().configure(DTU::SYSC_SEP, _sysc_label, 0, _sysc_epid, _sysc_credits, SYSC_RBUF_ORDER);

    DTU::get().start();
}

void *Env::mem() {
    if(_mem == nullptr) {
        _mem = mmap(0, MEM_SIZE, PROT_READ | PROT_WRITE, MAP_ANONYMOUS | MAP_PRIVATE, -1, 0);
        if(_mem == MAP_FAILED)
            PANIC("Unable to map heap");
    }
    return _mem;
}

void Env::print() const {
    char **env = environ;
    while(*env) {
        if(strstr(*env, "M3_") != nullptr || strstr(*env, "LD_") != nullptr) {
            char *dup = strdup(*env);
            char *name = strtok(dup, "=");
            printf("%s = %s\n", name, getenv(name));
            free(dup);
        }
        env++;
    }
}

Env::~Env() {
    delete _backend;
    if(is_kernel())
        stop_dtu();
}

}
