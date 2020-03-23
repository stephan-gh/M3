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

#pragma once

#include <base/Common.h>
#include <base/Config.h>
#include <base/EnvBackend.h>
#include <base/PEDesc.h>

namespace m3 {

class OStream;

class Env;
OStream &operator<<(OStream &, const Env &senv);

class Gem5EnvBackend : public EnvBackend {
    friend class Env;

public:
    virtual void init() = 0;
    virtual void reinit() = 0;
};

struct BootEnv {
    uint64_t pe_id;
    uint32_t pe_desc;
    uint32_t argc;
    uint64_t argv;
    uint64_t heap_size;
    uint64_t pe_mem_base;
    uint64_t pe_mem_size;
    uint64_t kenv;
} PACKED;

class Env : public BootEnv {
    friend OStream &operator<<(OStream &, const Env &senv);

public:
    uint64_t sp;
    uint64_t entry;
    uint64_t shared;
    uint64_t first_std_ep;
    uint64_t first_sel;

    uint64_t lambda;

    uint64_t rmng_sel;
    uint64_t pager_sess;

    uint64_t mounts_addr;
    uint64_t mounts_len;

    uint64_t fds_addr;
    uint64_t fds_len;

    uint64_t rbuf_cur;
    uint64_t rbuf_end;

    uint64_t vpe_addr;
    uint64_t backend_addr;

    Gem5EnvBackend *backend() {
        return reinterpret_cast<Gem5EnvBackend*>(backend_addr);
    }

    static void run() asm("env_run");

    void exit(int code, bool abort) NORETURN;

private:
    void pre_init();
    void post_init();
    void pre_exit();
} PACKED;

#define ENV_SPACE_SIZE           (ENV_SIZE - (sizeof(word_t) * 2 + sizeof(m3::Env)))
#define ENV_SPACE_START          (ENV_START + sizeof(m3::Env))
#define ENV_SPACE_END            (ENV_SPACE_START + ENV_SPACE_SIZE)

static inline Env *env() {
    return reinterpret_cast<Env*>(ENV_START);
}

}
