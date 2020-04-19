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

#include <utility>

namespace m3 {

/**
 * The different types of PEs
 */
enum class PEType {
    // Compute PE with internal memory
    COMP_IMEM   = 0,
    // Compute PE with cache and external memory
    COMP_EMEM   = 1,
    // memory PE
    MEM         = 2,
};

/**
 * The different ISAs
 */
enum class PEISA {
    NONE            = 0,
    X86             = 1,
    ARM             = 2,
    RISCV           = 3,
    ACCEL_INDIR     = 4,
    ACCEL_COPY      = 5,
    ACCEL_ROT13     = 6,
    IDE_DEV         = 7,
    NIC             = 8
};

/**
 * @return the number of supported ISAs
 */
static constexpr size_t isa_count() {
    return static_cast<size_t>(PEISA::NIC) + 1;
}

/**
 * Describes a PE
 */
struct PEDesc {
    typedef uint32_t value_t;

    /**
     * Default constructor
     */
    explicit PEDesc() : _value() {
    }
    /**
     * Creates a PE description from the given descriptor word
     */
    explicit PEDesc(value_t value) : _value(value) {
    }
    /**
     * Creates a PE description of given type, ISA and memory size
     */
    explicit PEDesc(PEType type, PEISA isa, size_t memsize = 0)
        : _value(static_cast<value_t>(type) | (static_cast<value_t>(isa) << 3) | memsize) {
    }

    /**
     * @return the raw descriptor word
     */
    value_t value() const {
        return _value;
    }

    /**
     * @return the type of PE
     */
    PEType type() const {
        return static_cast<PEType>(_value & 0x7);
    }
    /**
     * @return the isa of the PE
     */
    PEISA isa() const {
        return static_cast<PEISA>((_value >> 3) & 0xF);
    }
    /**
     * @return if the PE has a core that is programmable
     */
    bool is_programmable() const {
        return isa() != PEISA::NONE && isa() < PEISA::ACCEL_INDIR;
    }
    /**
     * @return if the PE supports multiple contexts
     */
    bool is_device() const {
        return isa() == PEISA::NIC || isa() == PEISA::IDE_DEV;
    }

    /**
     * @return if the PE supports VPEs
     */
    bool supports_vpes() const {
        return type() != PEType::MEM;
    }
    /**
     * @return if the PE supports the context switching protocol
     */
    bool supports_pemux() const {
        return supports_vpes() && !is_device();
    }

    /**
     * @return the memory size (for type() == COMP_IMEM | MEM)
     */
    size_t mem_size() const {
        return _value & ~static_cast<value_t>(0xFFF);
    }
    /**
     * @return true if the PE has internal memory
     */
    bool has_memory() const {
        return type() == PEType::COMP_IMEM || type() == PEType::MEM;
    }
    /**
     * @return true if the PE has a cache, i.e., external memory
     */
    bool has_cache() const {
        return type() == PEType::COMP_EMEM;
    }
    /**
     * @return true if the PE has virtual memory support of some form
     */
    bool has_virtmem() const {
        return has_cache();
    }

    /**
     * @return the starting address and size of the standard receive buffer space
     */
    std::pair<uintptr_t, size_t> rbuf_std_space() const {
        if(has_virtmem())
            return std::make_pair(RBUF_STD_ADDR, RBUF_STD_SIZE);
#if defined(__gem5__)
        return std::make_pair(mem_size() - RBUF_SIZE_SPM - RBUF_STD_SIZE, RBUF_STD_SIZE);
#else
        return std::make_pair(RBUF_STD_ADDR, RBUF_STD_SIZE);
#endif
    }

    /**
     * @return the starting address and size of the receive buffer space
     */
    std::pair<uintptr_t, size_t> rbuf_space() const {
        if(has_virtmem())
            return std::make_pair(RBUF_ADDR, RBUF_SIZE);
#if defined(__gem5__)
        return std::make_pair(mem_size() - RBUF_SIZE_SPM, RBUF_SIZE_SPM);
#else
        return std::make_pair(RBUF_ADDR, RBUF_SIZE);
#endif
    }

private:
    value_t _value;
} PACKED;

}
