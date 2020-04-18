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

#include <base/util/BitField.h>
#include <base/util/Math.h>
#include <base/util/Reference.h>
#include <base/util/String.h>
#include <base/ELF.h>
#include <base/Errors.h>
#include <base/KIF.h>
#include <base/PEDesc.h>

#include <m3/com/EPMng.h>
#include <m3/com/MemGate.h>
#include <m3/com/SendGate.h>
#include <m3/pes/KMem.h>
#include <m3/pes/PE.h>
#include <m3/session/Pager.h>
#include <m3/ObjCap.h>

#include <functional>
#include <memory>

namespace m3 {

class VPE;
class VFS;
class FileTable;
class MountTable;
class ResMng;
class FStream;
class EnvUserBackend;
class RecvGate;
class ClientSession;

class VPEArgs {
    friend class VPE;

public:
    explicit VPEArgs() noexcept;

    VPEArgs &pager(Reference<Pager> pager) noexcept;
    VPEArgs &resmng(ResMng *resmng) noexcept {
        _rmng = resmng;
        return *this;
    }
    VPEArgs &kmem(Reference<KMem> kmem) noexcept {
        _kmem = kmem;
        return *this;
    }

private:
    ResMng *_rmng;
    Reference<Pager> _pager;
    Reference<KMem> _kmem;
};

/**
 * Represents a virtual processing element which has been assigned to a PE. It will be under your
 * control in the sense that you can run arbitrary programs on it, exchange capabilities, wait until
 * a program on it finished and so on. You can also execute multiple programs in a row on it.
 *
 * Note that you have an object for your own VPE, but you can't use it to exchange capabilities or
 * execute programs in it. You can access the memory to derive sub areas from it, though.
 */
class VPE : public ObjCap {
    friend class EnvUserBackend;
    friend class CapRngDesc;
    friend class RecvGate;
    friend class ClientSession;
    friend class VFS;

    static const size_t BUF_SIZE;

    explicit VPE();

public:
    /**
     * @return your own VPE
     */
    static VPE &self() noexcept {
        return *_self_ptr;
    }

    explicit VPE(const Reference<class PE> &pe, const String &name, const VPEArgs &args = VPEArgs());
    virtual ~VPE();

    /**
     * @return the PE this VPE has been assigned to
     */
    const Reference<class PE> &pe() const noexcept {
        return _pe;
    }

    /**
     * @return the PE description this VPE has been assigned to
     */
    const PEDesc &pe_desc() const noexcept {
        return _pe->desc();
    }

    /**
     * @return the pager of this VPE (or nullptr)
     */
    Reference<Pager> &pager() noexcept {
        return _pager;
    }

    /**
     * @return the resource manager
     */
    std::unique_ptr<ResMng> &resmng() noexcept {
        return _resmng;
    }

    /**
     * @return the mount table
     */
    std::unique_ptr<MountTable> &mounts() noexcept {
        return _ms;
    }

    /**
     * @return the kernel memory quota
     */
    const Reference<KMem> &kmem() const noexcept {
        return _kmem;
    }

    /**
     * Clones the given mount table into this VPE.
     *
     * @param ms the mount table
     */
    void mounts(const std::unique_ptr<MountTable> &ms) noexcept;

    /**
     * Lets this VPE obtain all mount points in its mount table, i.e., the required capability
     * exchanges are performed.
     */
    void obtain_mounts();

    /**
     * @return the file descriptors
     */
    std::unique_ptr<FileTable> &fds() noexcept {
        return _fds;
    }

    /**
     * Clones the given file descriptors into this VPE. Note that the file descriptors depend
     * on the mount table, so that you should always prepare the mount table first.
     *
     * @param fds the file descriptors
     */
    void fds(const std::unique_ptr<FileTable> &fds) noexcept;

    /**
     * Lets this VPE obtain all files in its file table, i.e., the required capability exchanges
     * are performed.
     */
    void obtain_fds();

    /**
     * Allocates capability selectors.
     *
     * @param count the number of selectors
     * @return the first one
     */
    capsel_t alloc_sels(uint count) noexcept {
        _next_sel += count;
        return _next_sel - count;
    }
    capsel_t alloc_sel() noexcept {
        return _next_sel++;
    }

    /**
     * @return the endpoint manager for this VPE
     */
    EPMng &epmng() {
        return _epmng;
    }

    /**
     * @return the local memory of the PE this VPE is attached to
     */
    MemGate &mem() noexcept {
        return _mem;
    }
    const MemGate &mem() const noexcept {
        return _mem;
    }

    /**
     * Delegates the given object capability to this VPE.
     *
     * @param sel the selector
     */
    void delegate_obj(capsel_t sel) {
        delegate(KIF::CapRngDesc(KIF::CapRngDesc::OBJ, sel));
    }

    /**
     * Delegates the given range of capabilities to this VPE. They are put at the same selectors.
     *
     * @param crd the capabilities of your to VPE to delegate to this VPE
     */
    void delegate(const KIF::CapRngDesc &crd) {
        delegate(crd, crd.start());
    }

    /**
     * Delegates the given range of capabilities to this VPE at position <dest>.
     *
     * @param crd the capabilities of your to VPE to delegate to this VPE
     * @param dest the destination in this VPE
     */
    void delegate(const KIF::CapRngDesc &crd, capsel_t dest);

    /**
     * Obtains the given range of capabilities from this VPE to your VPE. The selectors are
     * automatically chosen.
     *
     * @param crd the capabilities of this VPE to delegate to your VPE
     */
    void obtain(const KIF::CapRngDesc &crd);

    /**
     * Obtains the given range of capabilities from this VPE to your VPE at position <dest>.
     *
     * @param crd the capabilities of this VPE to delegate to your VPE
     * @param dest the destination in your VPE
     */
    void obtain(const KIF::CapRngDesc &crd, capsel_t dest);

    /**
     * Revokes the given range of capabilities from this VPE.
     *
     * @param crd the capabilities to revoke
     * @param delonly whether to revoke delegations only
     */
    void revoke(const KIF::CapRngDesc &crd, bool delonly = false);

    /**
     * Starts the VPE, i.e., prepares the PE for execution and wakes it up.
     */
    void start();

    /**
     * Stops the VPE, i.e., if it is running, the execution is stopped.
     */
    void stop();

    /**
     * Waits until the currently executing program on this VPE is finished
     *
     * @return the exitcode
     */
    int wait();

    /**
     * Starts to wait until the currently executing program on this VPE is finished, but tells to
     * kernel to notify us asynchronously via upcall.
     *
     * @param event the event for the upcall
     * @return 0 on success
     */
    int wait_async(event_t event);

    /**
     * Executes the given program on this VPE.
     *
     * @param argc the number of arguments to pass to the program
     * @param argv the arguments to pass (argv[0] is the executable)
     */
    void exec(int argc, const char **argv);

    /**
     * Clones this program onto this VPE and executes the given function.
     *
     * @param f the function to execute
     */
    void run(std::function<int()> f) {
        std::unique_ptr<std::function<int()>> copy(new std::function<int()>(f));
        run(copy.get());
    }

private:
    void mark_caps_allocated(capsel_t sel, uint count) {
        _next_sel = Math::max(_next_sel, sel + count);
    }

    static void reset() noexcept;

    void init_state();
    void init_fs();
    void run(void *lambda);
    void load_segment(ElfPh &pheader, char *buffer);
    void load(int argc, const char **argv, uintptr_t *entry, char *buffer, size_t *size);
    void clear_mem(char *buffer, size_t count, uintptr_t dest);
    size_t store_arguments(char *buffer, int argc, const char **argv);

    uintptr_t get_entry();
    static bool skip_section(ElfPh *ph);
    void copy_sections();

    Reference<class PE> _pe;
    Reference<KMem> _kmem;
    MemGate _mem;
    capsel_t _next_sel;
    epid_t _eps_start;
    EPMng _epmng;
    Reference<Pager> _pager;
    std::unique_ptr<ResMng> _resmng;
    std::unique_ptr<MountTable> _ms;
    std::unique_ptr<FileTable> _fds;
    std::unique_ptr<FStream> _exec;
    static VPE _self;
    static VPE *_self_ptr;
};

}
