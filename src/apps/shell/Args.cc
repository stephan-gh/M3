/*
 * Copyright (C) 2015-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019 Nils Asmussen, Barkhausen Institut
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

#include "Args.h"

#include <m3/vfs/Dir.h>

#include <sstream>
#include <string.h>

#include "Builtin.h"
#include "Parser.h"
#include "Vars.h"

using namespace m3;

int Args::strmatch(const char *pattern, const char *str) {
    const char *lastStar;
    char *firstStar = const_cast<char *>(strchr(pattern, '*'));
    if(firstStar == NULL)
        return strcmp(pattern, str) == 0;
    lastStar = strrchr(pattern, '*');
    /* does the beginning match? */
    if(firstStar != pattern) {
        if(strncmp(str, pattern, static_cast<size_t>(firstStar - pattern)) != 0)
            return false;
    }
    /* does the end match? */
    if(lastStar[1] != '\0') {
        size_t plen = strlen(pattern);
        size_t slen = strlen(str);
        size_t cmplen = static_cast<size_t>(pattern + plen - lastStar - 1);
        if(strncmp(lastStar + 1, str + slen - cmplen, cmplen) != 0)
            return false;
    }

    /* now check whether the parts between the stars match */
    str += firstStar - pattern;
    while(1) {
        const char *match;
        const char *start = firstStar + 1;
        firstStar = const_cast<char *>(strchr(start, '*'));
        if(firstStar == NULL)
            break;

        *firstStar = '\0';
        match = strstr(str, start);
        *firstStar = '*';
        if(match == NULL)
            return false;
        str = match + (firstStar - start);
    }
    return true;
}

void Args::glob(std::unique_ptr<Parser::ArgList> &list, size_t i) {
    char filepat[MAX_ARG_LEN];
    char dirpath[256];
    const char *pat = expr_value(*list->get(i));
    const char *slash = strrchr(pat, '/');
    if(slash) {
        strcpy(filepat, slash + 1);
        strncpy(dirpath, pat, static_cast<size_t>(slash + 1 - pat));
        dirpath[slash + 1 - pat] = '\0';
    }
    else {
        strcpy(filepat, pat);
        strcpy(dirpath, "");
    }
    size_t patlen = strlen(dirpath);

    Dir dir(dirpath);
    Dir::Entry e;
    bool found = false;
    while(dir.readdir(e)) {
        if(strcmp(e.name, ".") == 0 || strcmp(e.name, "..") == 0)
            continue;

        if(strmatch(filepat, e.name)) {
            if(patlen + strlen(e.name) + 1 <= MAX_ARG_LEN) {
                std::ostringstream os;
                os << dirpath << e.name;
                if(found)
                    list->insert(i, std::make_unique<Parser::Expr>(os.str(), false));
                else
                    list->replace(i, std::make_unique<Parser::Expr>(os.str(), false));
                i++;
                found = true;
            }
        }
    }

    // remove wildcard argument if we haven't found anything
    if(!found)
        list->remove(i);
}

void Args::prefix_path(std::unique_ptr<Parser::ArgList> &list) {
    if(list->size() == 0)
        return;

    const char *first = expr_value(*list->get(0));
    if(first[0] != '/' && !Builtin::is_builtin(first)) {
        std::ostringstream os;
        os << "/bin/" << first;
        list->replace(0, std::make_unique<Parser::Expr>(os.str(), false));
    }
}

void Args::expand(std::unique_ptr<Parser::ArgList> &list) {
    for(size_t i = 0; i < list->size(); ++i) {
        if(strchr(expr_value(*list->get(i)), '*'))
            glob(list, i);
    }
}
