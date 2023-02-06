/*
 * Copyright (C) 2022 Nils Asmussen, Barkhausen Institut
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

#include <base/util/Math.h>

#include <m3/Env.h>
#include <m3/EnvVars.h>

#include <algorithm>
#include <assert.h>
#include <sstream>
#include <stdlib.h>
#include <string.h>

namespace m3 {

extern "C" char **__environ;

static bool copied = false;

static void env_vars_dealloc() {
    assert(copied);
    char **e = __environ;
    for(size_t i = 0; e && *e; ++e, ++i)
        free(__environ[i]);
    free(__environ);
    __environ = nullptr;
}

void EnvVars::append(char *pair) {
    size_t total = count();
    // we need two more slots; the new var and null-termination
    __environ = static_cast<char **>(realloc(__environ, (total + 2) * sizeof(char *)));
    assert(__environ != nullptr);
    __environ[total] = pair;
    __environ[total + 1] = nullptr;
}

void EnvVars::copy() {
    if(!copied) {
        copied = true;
        char **old = __environ;
        // allocate array with sufficient slots
        size_t total = count();
        __environ = static_cast<char **>(malloc((total + 1) * sizeof(char *)));
        assert(__environ != nullptr);

        // add vars
        char **e = old;
        for(size_t i = 0; e && *e; ++e, ++i)
            __environ[i] = strdup(*e);
        __environ[total] = nullptr;
        atexit(env_vars_dealloc);
    }
}

char **EnvVars::find_var(const char *key, size_t key_len) {
    char **e = __environ;
    for(; e && *e; ++e) {
        if(strncmp(*e, key, key_len) == 0 && (*e)[key_len] == '=')
            return e;
    }
    return nullptr;
}

size_t EnvVars::count() {
    // always count them, because the musl implementation could have changed it in the meantime
    char **e = __environ;
    while(e && *e)
        e++;
    return static_cast<size_t>(e - __environ);
}

const char *const *EnvVars::vars() {
    return __environ;
}

const char *EnvVars::get(const char *key) {
    size_t key_len = strlen(key);
    char **var = find_var(key, key_len);
    if(var)
        return (*var) + key_len + 1;
    return nullptr;
}

void EnvVars::set(const char *key, const char *value) {
    assert(strchr(key, '=') == nullptr);
    // adding/changing requires a copy
    copy();

    // create new entry
    size_t key_len = strlen(key);
    char *nvar = static_cast<char *>(malloc(key_len + strlen(value) + 2));
    assert(nvar != nullptr);
    strcpy(nvar, key);
    nvar[key_len] = '=';
    strcpy(nvar + key_len + 1, value);

    // replace or append it
    char **var = find_var(key, key_len);
    if(var) {
        free(*var);
        *var = nvar;
    }
    else
        append(nvar);
}

void EnvVars::remove(const char *key) {
    assert(strchr(key, '=') == nullptr);
    // removing requires a copy
    copy();

    char **var = find_var(key, strlen(key));
    if(var) {
        size_t total = count();
        free(*var);
        // move following backwards
        size_t following = static_cast<size_t>(var - __environ);
        memmove(var, var + 1, (total - following - 1) * sizeof(char *));
        // null-termination
        __environ[total - 1] = nullptr;
    }
}

}
