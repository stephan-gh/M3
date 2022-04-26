/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <m3/EnvVars.h>

#include "Parser.h"

#include <stdlib.h>
#include <string.h>
#include <vector>

class Vars {
public:
    explicit Vars() : _vars() {
        for(size_t i = 0; i < m3::EnvVars::count(); ++i) {
            const char *var = m3::EnvVars::vars()[i];
            char *copy = static_cast<char*>(malloc(strlen(var) + 1));
            strcpy(copy, var);
            _vars.push_back(copy);
        }
        _vars.push_back(nullptr);
    }
    ~Vars() {
        for(auto it = _vars.begin(); *it != nullptr; ++it)
            free(const_cast<char*>(*it));
    }

    const char *const *get() const {
        return _vars.data();
    }

    void set(const char *name, const char *value) {
        for(auto it = _vars.begin(); *it != nullptr; ++it) {
            size_t eq_pos = static_cast<size_t>(strchr(*it, '=') - *it);
            if(strncmp(*it, name, eq_pos) == 0 && name[eq_pos] == '\0') {
                free(const_cast<char*>(*it));
                *it = build_var(name, value);
                return;
            }
        }

        _vars[_vars.size() - 1] = build_var(name, value);
        _vars.push_back(nullptr);
    }

private:
    char *build_var(const char *name, const char *value) {
        size_t name_len = strlen(name);
        char *nvar = static_cast<char*>(malloc(name_len + strlen(value) + 2));
        strcpy(nvar, name);
        nvar[name_len] = '=';
        strcpy(nvar + name_len + 1, value);
        return nvar;
    }

    std::vector<const char*> _vars;
};

static inline const char *expr_value(Expr *e) {
    if(e->is_var) {
        const char *eval = m3::EnvVars::get(e->name_val);
        return eval ? eval : "";
    }
    return e->name_val;
}
