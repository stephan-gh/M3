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

#include <m3/ObjCap.h>

namespace m3 {

/**
 * A semaphore allows synchronization of different VPEs, based on system calls
 */
class Semaphore : public ObjCap {
    Semaphore(capsel_t sel, uint flags) noexcept
        : ObjCap(SEM, sel, flags) {
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

    Semaphore(Semaphore &&sem) noexcept
        : ObjCap(Util::move(sem)) {
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
