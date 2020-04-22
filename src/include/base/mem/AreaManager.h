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

#include <base/stream/OStream.h>
#include <base/util/Math.h>

#include <utility>

namespace m3 {

struct Area {
    goff_t addr;
    size_t size;
    Area *next;
};

template<class A = Area>
class AreaManager {
public:
    /**
     * Creates a memory-map for given area
     *
     * @param area the area of address and size
     */
    explicit AreaManager(std::pair<goff_t, size_t> area) : AreaManager(area.first, area.second) {
    }

    /**
     * Creates a memory-map of <size> bytes.
     *
     * @param addr the base address
     * @param size the mem size
     */
    explicit AreaManager(goff_t addr, size_t size) : list(new A()) {
        list->addr = addr;
        list->size = size;
        list->next = nullptr;
    }

    /**
     * Destroys this map
     */
    ~AreaManager() {
        for(A *a = list; a != nullptr;) {
            A *n = static_cast<A*>(a->next);
            delete a;
            a = n;
        }
        list = nullptr;
    }

    /**
     * Allocates an area in the given map, that is <size> bytes large.
     *
     * @param map the map
     * @param size the size of the area
     * @param align the desired alignment
     * @return the address of -1 if failed
     */
    goff_t allocate(size_t size, size_t align) {
        A *a;
        A *p = nullptr;
        for(a = list; a != nullptr; p = a, a = static_cast<A*>(a->next)) {
            size_t diff = m3::Math::round_up(a->addr, static_cast<goff_t>(align)) - a->addr;
            if(a->size > diff && a->size - diff >= size)
                break;
        }
        if(a == nullptr)
            return static_cast<goff_t>(-1);

        /* if we need to do some alignment, create a new area in front of a */
        size_t diff = m3::Math::round_up(a->addr, static_cast<goff_t>(align)) - a->addr;
        if(diff) {
            A *n = new A();
            n->addr = a->addr;
            n->size = diff;
            if(p)
                p->next = n;
            else
                list = n;
            n->next = a;

            a->addr += diff;
            a->size -= diff;
            p = n;
        }

        /* take it from the front */
        goff_t res = a->addr;
        a->size -= size;
        a->addr += size;
        /* if the area is empty now, remove it */
        if(a->size == 0) {
            if(p)
                p->next = a->next;
            else
                list = static_cast<A*>(a->next);
            delete a;
        }
        return res;
    }

    /**
     * Frees the area at <addr> with <size> bytes.
     *
     * @param map the map
     * @param addr the address of the area
     * @param size the size of the area
     */
    void free(goff_t addr, size_t size) {
        /* find the area behind ours */
        A *n, *p = nullptr;
        for(n = list; n != nullptr && addr > n->addr; p = n, n = static_cast<A*>(n->next))
            ;

        /* merge with prev and next */
        if(p && p->addr + p->size == addr && n && addr + size == n->addr) {
            p->size += size + n->size;
            p->next = n->next;
            delete n;
        }
        /* merge with prev */
        else if(p && p->addr + p->size == addr) {
            p->size += size;
        }
        /* merge with next */
        else if(n && addr + size == n->addr) {
            n->addr -= size;
            n->size += size;
        }
        /* create new area between them */
        else {
            A *a = new A();
            a->addr = addr;
            a->size = size;
            if(p)
                p->next = a;
            else
                list = a;
            a->next = n;
        }
    }

    /**
     * Just for debugging/testing: Determines the total number of free bytes in the map
     *
     * @param map the map
     * @param areas will be set to the number of areas in the map
     * @return the free bytes
     */
    size_t get_size(size_t *areas = nullptr) const {
        size_t total = 0;
        if(areas)
            *areas = 0;
        for(A *a = list; a != nullptr; a = static_cast<A*>(a->next)) {
            total += a->size;
            if(areas)
                (*areas)++;
        }
        return total;
    }

    friend m3::OStream &operator<<(m3::OStream &os, const AreaManager &map) {
        size_t areas;
        os << "Total: " << (map.get_size(&areas) / 1024) << " KiB:\n";
        for(A *a = map.list; a != nullptr; a = a->next)
            os << "\t@ " << m3::fmt(a->addr, "p") << ", " << (a->size / 1024) << " KiB\n";
        return os;
    }

private:
    A *list;
};

}