/*
 * Copyright (C) 2016-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <m3/Syscalls.h>
#include <m3/com/Semaphore.h>
#include <m3/session/ResMng.h>
#include <m3/tiles/Activity.h>

namespace m3 {

Semaphore Semaphore::attach(const char *name) {
    capsel_t nsel = SelSpace::get().alloc_sel();
    Activity::own().resmng()->use_sem(nsel, name);
    return Semaphore(nsel, KEEP_CAP);
}

Semaphore Semaphore::create(uint value) {
    capsel_t nsel = SelSpace::get().alloc_sel();
    Syscalls::create_sem(nsel, value);
    return Semaphore(nsel, 0);
}

void Semaphore::up() const {
    Syscalls::sem_ctrl(sel(), KIF::Syscall::SCTRL_UP);
}

void Semaphore::down() const {
    Syscalls::sem_ctrl(sel(), KIF::Syscall::SCTRL_DOWN);
}

}
