/*
 * Copyright (C) 2016, Nils Asmussen <nils@os.inf.tu-dresden.de>
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
#include <base/stream/Serial.h>
#include <base/CPU.h>
#include <base/Env.h>

#include <exception>
#include <functional>
#include <stdlib.h>

typedef void (*constr_func)();

extern constr_func CTORS_BEGIN;
extern constr_func CTORS_END;

EXTERN_C void __cxa_finalize(void *);
EXTERN_C void _init();
EXTERN_C void init_env(m3::Env *env);
EXTERN_C int main(int argc, char **argv);

namespace m3 {

OStream &operator<<(OStream &os, const Env &senv) {
    os << "pe_id        : " << senv.pe_id << "\n";
    os << "pe_desc      : " << fmt(senv.pe_desc, "#x") << "\n";
    os << "argc         : " << senv.argc << "\n";
    os << "argv         : " << fmt(senv.argv, "p") << "\n";
    os << "heap_size    : " << fmt(senv.heap_size, "#x") << "\n";
    os << "pe_mem_base  : " << fmt(senv.pe_mem_base, "p") << "\n";
    os << "pe_mem_size  : " << fmt(senv.pe_mem_size, "#x") << "\n";
    os << "sp           : " << fmt(senv.sp, "p") << "\n";
    os << "entry        : " << fmt(senv.entry, "p") << "\n";
    os << "first_std_ep : " << senv.first_std_ep << "\n";
    os << "first_sel    : " << senv.first_sel << "\n";
    os << "lambda       : " << fmt(senv.lambda, "p") << "\n";
    os << "rmng_sel     : " << senv.rmng_sel << "\n";
    os << "pager_sess   : " << senv.pager_sess << "\n";
    os << "mounts_addr  : " << fmt(senv.mounts_addr, "p") << "\n";
    os << "mounts_len   : " << senv.mounts_len << "\n";
    os << "fds_addr     : " << senv.fds_addr << "\n";
    os << "fds_len      : " << fmt(senv.fds_len, "p") << "\n";
    os << "rbuf_cur     : " << fmt(senv.rbuf_cur, "p") << "\n";
    os << "rbuf_end     : " << fmt(senv.rbuf_end, "p") << "\n";
    os << "backend_addr : " << fmt(senv.backend_addr, "p") << "\n";
    return os;
}

void Env::pre_init() {
}

void Env::post_init() {
    // call constructors
    _init();
    for(constr_func *func = &CTORS_BEGIN; func < &CTORS_END; ++func)
        (*func)();
}

void Env::pre_exit() {
}

void Env::run() {
    Env *e = env();

    int res;
    if(e->lambda) {
        e->backend()->reinit();

        std::function<int()> *f = reinterpret_cast<std::function<int()>*>(e->lambda);
        res = (*f)();
    }
    else {
        init_env(e);
        e->pre_init();
        e->backend()->init();
        e->post_init();

        char **argv = reinterpret_cast<char**>(e->argv);
        if(sizeof(char*) != sizeof(uint64_t)) {
            uint64_t *argv64 = reinterpret_cast<uint64_t*>(e->argv);
            argv = new char*[e->argc];
            for(uint64_t i = 0; i < e->argc; ++i)
                argv[i] = reinterpret_cast<char*>(argv64[i]);
        }
        res = main(static_cast<int>(e->argc), argv);
    }

    e->exit(res, false);
    UNREACHED;
}

USED void Env::exit(int code, bool abort) {
    pre_exit();
    if(!abort)
        __cxa_finalize(nullptr);
    backend()->exit(code);
    UNREACHED;
}

}
