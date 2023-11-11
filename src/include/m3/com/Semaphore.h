/*
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

#pragma once

#include <m3/cap/ObjCap.h>

#include <utility>

namespace m3 {

/**
 * A semaphore allows synchronization of different activities, based on system calls
 */
class Semaphore : public ObjCap {
    Semaphore(capsel_t sel, uint flags) noexcept : ObjCap(SEM, sel, flags) {
    }

public:
    /**
     * Attaches to the semaphore associated with <name> by the resource manager
     *
     * @param name the name of the semaphore
     * @return the semaphore
     */
    static Semaphore attach(const char *name);

    /**
     * Creates a new semaphore with given value
     *
     * @param value the semaphores initial value
     * @return the semaphore
     */
    static Semaphore create(uint value);

    /**
     * Binds a Semaphore object to the given selector
     *
     * @param sel the selector of an existing semaphore
     * @return the semaphore
     */
    static Semaphore bind(capsel_t sel) {
        return Semaphore(sel, KEEP_CAP);
    }

    Semaphore(Semaphore &&sem) noexcept : ObjCap(std::move(sem)) {
    }

    /**
     * Increase the value by one.
     */
    void up() const;

    /**
     * Decrease the value by one.
     */
    void down() const;
};

}
