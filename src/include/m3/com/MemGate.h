/*
 * Copyright (C) 2015-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#pragma once

#include <base/KIF.h>

#include <m3/com/Gate.h>

namespace pci {
class ProxiedPciDevice;
}

namespace m3 {

class Activity;

/**
 * A memory gate is used to access tile-external memory via the TCU. You can either create a MemGate
 * by requesting tile-external memory from the kernel or bind a MemGate to an existing capability.
 */
class MemGate : public Gate {
    friend class AladdinAccel;
    friend class InDirAccel;
    friend class StreamAccel;
    friend class pci::ProxiedPciDevice;

    explicit MemGate(uint flags, capsel_t cap, bool revoke) noexcept
        : Gate(MEM_GATE, cap, flags),
          _revoke(revoke) {
    }

public:
    static const int R = KIF::Perm::R;
    static const int W = KIF::Perm::W;
    static const int X = KIF::Perm::X;
    static const int RW = R | W;
    static const int RWX = R | W | X;
    static const int PERM_BITS = 3;

    /**
     * Creates a new memory gate for global memory. That is, it requests <size> bytes of global
     * memory with given permissions.
     *
     * @param size the memory size
     * @param perms the permissions (see MemGate::RWX)
     * @param sel the selector to use (if != INVALID, the selector is NOT freed on destruction)
     * @param flags the flags to control whether the cap is kept
     * @return the memory gate
     */
    static MemGate create_global(size_t size, int perms, capsel_t sel = INVALID, uint flags = 0);

    /**
     * Binds a new memory-gate to the boot module with given name.
     *
     * @param name the name of the boot module
     * @return the memory gate
     */
    static MemGate bind_bootmod(const std::string_view &name);

    /**
     * Binds this gate for read/write/cmpxchg to the given memory capability. That is, the
     * capability should be a memory capability you've received from somebody else.
     *
     * @param sel the capability selector
     * @param flags the flags to control whether the cap is kept
     */
    static MemGate bind(capsel_t sel, uint flags = ObjCap::KEEP_CAP) noexcept {
        return MemGate(flags, sel, true);
    }

    MemGate(MemGate &&m) noexcept : Gate(std::move(m)), _revoke(m._revoke) {
    }

    ~MemGate();

    /**
     * Derives memory from this memory gate. That is, it creates a new memory capability that is
     * bound to a subset of this memory (in space or permissions).
     *
     * @param offset the offset inside this memory capability
     * @param size the size of the memory area
     * @param perms the permissions (you can only downgrade)
     * @return the new memory gate
     */
    MemGate derive(goff_t offset, size_t size, int perms = RWX) const;

    /**
     * Derives memory from this memory gate for <act> and uses <sel> for it. That is, it creates
     * a new memory capability that is bound to a subset of this memory (in space or permissions).
     *
     * @param act the activity to delegate the derived cap to
     * @param sel the capability selector to use
     * @param offset the offset inside this memory capability
     * @param size the size of the memory area
     * @param perms the permissions (you can only downgrade)
     * @param flags the capability flags
     * @return the new memory gate
     */
    MemGate derive_for(capsel_t act, capsel_t sel, goff_t offset, size_t size, int perms = RWX,
                       uint flags = 0) const;

    /**
     * Writes the <len> bytes at <data> to <offset>.
     *
     * @param data the data to write
     * @param len the number of bytes to write
     * @param offset the start-offset
     */
    void write(const void *data, size_t len, goff_t offset);

    /**
     * Reads <len> bytes from <offset> into <data>.
     *
     * @param data the buffer to write into
     * @param len the number of bytes to read
     * @param offset the start-offset
     */
    void read(void *data, size_t len, goff_t offset);

private:
    bool _revoke;
};

}
