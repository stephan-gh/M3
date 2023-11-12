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

namespace m3 {

/**
 * A memory capability is the precursor of a MemGate.
 *
 * MemCap can be turned into a MemGate through activation.
 */
class MemCap : public ObjCap {
    explicit MemCap(uint flags, capsel_t cap, bool resmng) noexcept
        : ObjCap(MEM_GATE, cap, flags),
          _resmng(resmng) {
    }

public:
    static const int R = KIF::Perm::R;
    static const int W = KIF::Perm::W;
    static const int X = KIF::Perm::X;
    static const int RW = R | W;
    static const int RWX = R | W | X;

    /**
     * Creates a new memory capability for global memory. That is, it requests <size> bytes of
     * global memory with given permissions.
     *
     * @param size the memory size
     * @param perms the permissions (see MemCap::RWX)
     * @param sel the selector to use (if != INVALID, the selector is NOT freed on destruction)
     * @return the memory capability
     */
    static MemCap create_global(size_t size, int perms, capsel_t sel = INVALID);

    /**
     * Binds a new memory capability to the boot module with given name.
     *
     * @param name the name of the boot module
     * @return the memory capability
     */
    static MemCap bind_bootmod(const std::string_view &name);

    /**
     * Binds this capability for read/write/cmpxchg to the given memory capability. That is, the
     * capability should be a memory capability you've received from somebody else.
     *
     * @param sel the capability selector
     * @param flags the flags to control whether the cap is kept
     */
    static MemCap bind(capsel_t sel, uint flags = ObjCap::KEEP_CAP) noexcept {
        return MemCap(flags, sel, false);
    }

    MemCap(MemCap &&m) noexcept : ObjCap(std::move(m)), _resmng(m._resmng) {
    }

    ~MemCap();

    /**
     * Derives memory from this memory capability. That is, it creates a new memory capability that
     * is bound to a subset of this memory (in space or permissions).
     *
     * @param offset the offset inside this memory capability
     * @param size the size of the memory area
     * @param perms the permissions (you can only downgrade)
     * @return the new memory capability
     */
    MemCap derive(goff_t offset, size_t size, int perms = RWX) const;

    /**
     * Derives memory from this memory capability for <act> and uses <sel> for it. That is, it
     * creates a new memory capability that is bound to a subset of this memory (in space or
     * permissions).
     *
     * @param act the activity to delegate the derived cap to
     * @param sel the capability selector to use
     * @param offset the offset inside this memory capability
     * @param size the size of the memory area
     * @param perms the permissions (you can only downgrade)
     * @return the new memory capability
     */
    MemCap derive_for(capsel_t act, capsel_t sel, goff_t offset, size_t size,
                      int perms = RWX) const;

    /**
     * Activates this MemCap and thereby turns it into a usable MemGate
     *
     * This will allocate a new EP from the EPMng.
     *
     * @return the created MemGate
     */
    MemGate activate();

    /**
     * Activates this MemCap on the given EP for someone else
     *
     * As it will be used by someone else, no MemGate is returned.
     */
    void activate_on(const EP &ep);

private:
    bool _resmng;
};

/**
 * A memory gate is used to access tile-external memory via the TCU. You can either create a MemGate
 * by requesting tile-external memory from the kernel or bind a MemGate to an existing capability.
 */
class MemGate : public Gate {
    friend class MemCap;

    explicit MemGate(uint flags, capsel_t cap, bool resmng, EP *ep) noexcept
        : Gate(MEM_GATE, cap, flags, ep),
          _resmng(resmng) {
    }

public:
    typedef MemCap Cap;

    static const int R = MemCap::R;
    static const int W = MemCap::W;
    static const int X = MemCap::X;
    static const int RW = MemCap::RW;
    static const int RWX = MemCap::RWX;

    /**
     * Creates a new memory gate for global memory. That is, it requests <size> bytes of global
     * memory with given permissions.
     *
     * @param size the memory size
     * @param perms the permissions (see MemGate::RWX)
     * @param sel the selector to use (if != INVALID, the selector is NOT freed on destruction)
     * @return the memory gate
     */
    static MemGate create_global(size_t size, int perms, capsel_t sel = INVALID) {
        return MemCap::create_global(size, perms, sel).activate();
    }

    /**
     * Binds a new memory-gate to the boot module with given name.
     *
     * @param name the name of the boot module
     * @return the memory gate
     */
    static MemGate bind_bootmod(const std::string_view &name) {
        return MemCap::bind_bootmod(name).activate();
    }

    /**
     * Binds this gate for read/write/cmpxchg to the given memory capability. That is, the
     * capability should be a memory capability you've received from somebody else.
     *
     * @param sel the capability selector
     * @param flags the flags to control whether the cap is kept
     */
    static MemGate bind(capsel_t sel, uint flags = ObjCap::KEEP_CAP) {
        return MemCap::bind(sel, flags).activate();
    }

    MemGate(MemGate &&m) noexcept : Gate(std::move(m)), _resmng(m._resmng) {
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
    MemGate derive(goff_t offset, size_t size, int perms = RWX) const {
        return derive_cap(offset, size, perms).activate();
    }

    /**
     * Like derive(), but does not create a MemGate, but a MemCap
     */
    MemCap derive_cap(goff_t offset, size_t size, int perms = RWX) const {
        return MemCap::bind(sel()).derive(offset, size, perms);
    }

    /**
     * Derives memory from this memory gate for <act> and uses <sel> for it. That is, it creates
     * a new memory capability that is bound to a subset of this memory (in space or permissions).
     *
     * @param act the activity to delegate the derived cap to
     * @param sel the capability selector to use
     * @param offset the offset inside this memory capability
     * @param size the size of the memory area
     * @param perms the permissions (you can only downgrade)
     * @return the new memory gate
     */
    MemGate derive_for(capsel_t act, capsel_t sel, goff_t offset, size_t size,
                       int perms = RWX) const {
        return MemCap::bind(this->sel()).derive_for(act, sel, offset, size, perms).activate();
    }

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
    bool _resmng;
};

}
