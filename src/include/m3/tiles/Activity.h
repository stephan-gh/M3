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

#include <base/time/Instant.h>
#include <base/util/BitField.h>
#include <base/util/Math.h>
#include <base/util/Reference.h>
#include <base/util/String.h>
#include <base/ELF.h>
#include <base/Errors.h>
#include <base/KIF.h>
#include <base/TileDesc.h>
#include <base/TMIF.h>

#include <m3/com/EPMng.h>
#include <m3/com/MemGate.h>
#include <m3/com/SendGate.h>
#include <m3/com/Marshalling.h>
#include <m3/tiles/KMem.h>
#include <m3/tiles/Tile.h>
#include <m3/session/Pager.h>
#include <m3/ObjCap.h>

#include <functional>
#include <memory>

namespace m3 {

class Activity;
class VFS;
class FileTable;
class MountTable;
class ResMng;
class FStream;
class EnvUserBackend;
class RecvGate;
class ClientSession;

class ActivityArgs {
    friend class Activity;

public:
    explicit ActivityArgs() noexcept;

    ActivityArgs &pager(Reference<Pager> pager) noexcept;
    ActivityArgs &resmng(ResMng *resmng) noexcept {
        _rmng = resmng;
        return *this;
    }
    ActivityArgs &kmem(Reference<KMem> kmem) noexcept {
        _kmem = kmem;
        return *this;
    }

private:
    ResMng *_rmng;
    Reference<Pager> _pager;
    Reference<KMem> _kmem;
};

/**
 * Represents an activity on a tile. On general-purpose tiles, the activity executes code on the
 * core. On accelerator/device tiles, the activity uses the logic of the accelerator/device.
 *
 * Note that you have an object for your own activity, but you can't use it to exchange capabilities
 * or execute programs in it. You can access the memory to derive sub areas from it, though.
 */
class Activity : public ObjCap {
    friend class EnvUserBackend;
    friend class CapRngDesc;
    friend class RecvGate;
    friend class ClientSession;
    friend class VFS;

    static const size_t BUF_SIZE;
    static constexpr size_t DATA_SIZE = 256;

    explicit Activity();

public:
    /**
     * @return your own activity
     */
    static Activity &self() noexcept {
        return _self;
    }

    /**
     * Puts the current activity to sleep until the next message arrives
     */
    static void sleep() noexcept {
        sleep_for(TimeDuration::MAX);
    }

    /**
     * Puts the current activity to sleep until the next message arrives or <nanos> nanoseconds have
     * passed.
     */
    static void sleep_for(TimeDuration duration) noexcept {
        if(env()->shared || duration != TimeDuration::MAX)
            TMIF::wait(TCU::INVALID_EP, INVALID_IRQ, duration);
#if !defined(__host__)
        else if(env()->platform != Platform::HW)
            TCU::get().wait_for_msg(TCU::INVALID_EP);
#else
            TCU::get().wait_for_msg(TCU::INVALID_EP, duration.as_nanos());
#endif
    }

    /**
     * Puts the current activity to sleep until the next message arrives on the given EP
     */
    static void wait_for_msg(epid_t ep) noexcept {
        if(env()->shared)
            TMIF::wait(ep, INVALID_IRQ, TimeDuration::MAX);
#if !defined(__host__)
        else if(env()->platform != Platform::HW)
            TCU::get().wait_for_msg(ep);
#else
            TCU::get().wait_for_msg(TCU::INVALID_EP, 0);
#endif
    }

    explicit Activity(const Reference<class Tile> &tile, const String &name,
                      const ActivityArgs &args = ActivityArgs());
    virtual ~Activity();

    /**
     * @return the activity id (for debugging purposes)
     */
    actid_t id() const noexcept {
        return _id;
    }

    /**
     * @return the tile this activity has been assigned to
     */
    const Reference<class Tile> &tile() const noexcept {
        return _tile;
    }

    /**
     * @return the tile description this activity has been assigned to
     */
    const TileDesc &tile_desc() const noexcept {
        return _tile->desc();
    }

    /**
     * @return the pager of this activity (or nullptr)
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
     * @return the kernel memory quota
     */
    const Reference<KMem> &kmem() const noexcept {
        return _kmem;
    }

    /**
     * Returns the mounts of this activity.
     *
     * Mounts that are added to child activities are automatically delegated to the child upon
     * Activity::run and Activity::exec. For example:
     * <code>
     * child.mounts().add("/", Activity::cur().mounts().get_by_path("/").unwrap()));
     * </code>
     *
     * @return the mount table
     */
    std::unique_ptr<MountTable> &mounts() noexcept {
        return _ms;
    }

    /**
     * Returns the files of this activity.
     *
     * Files that are added to child activities are automatically delegated to the child upon Activity::run and
     * Activity::exec. For example, you can connect the child's STDOUT to one of your files in the
     * following way:
     * <code>
     * child.files()->set(STDOUT_FD, Activity::self().fds()->get(4));
     * </code>
     *
     * @return the files
     */
    std::unique_ptr<FileTable> &files() noexcept {
        return _fds;
    }

    /**
     * Returns a marshaller for the activity-local data.
     *
     * The marshaller overwrites the activity-local data and will be transmitted to the activity when calling
     * Activity::run or Activity::exec.
     *
     * @return a marshaller to write to the activity-local data
     */
    Marshaller data_sink() noexcept {
        return Marshaller(_data, sizeof(_data));
    }

    /**
     * Returns an unmarshaller for the activity-local data.
     *
     * The source provides access to the activity-local data that has been transmitted to this activity from
     * its parent during Activity::run or Activity::exec.
     *
     * @return an unmarshaller to read from the activity-local data
     */
    Unmarshaller data_source() noexcept {
        return Unmarshaller(_data, sizeof(_data));
    }

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
     * @return the endpoint manager for this activity
     */
    EPMng &epmng() {
        return _epmng;
    }

    /**
     * Delegates the given object capability to this activity.
     *
     * @param sel the selector
     */
    void delegate_obj(capsel_t sel) {
        delegate(KIF::CapRngDesc(KIF::CapRngDesc::OBJ, sel));
    }

    /**
     * Delegates the given range of capabilities to this activity. They are put at the same selectors.
     *
     * @param crd the capabilities of your to activity to delegate to this activity
     */
    void delegate(const KIF::CapRngDesc &crd) {
        delegate(crd, crd.start());
    }

    /**
     * Delegates the given range of capabilities to this activity at position <dest>.
     *
     * @param crd the capabilities of your to activity to delegate to this activity
     * @param dest the destination in this activity
     */
    void delegate(const KIF::CapRngDesc &crd, capsel_t dest);

    /**
     * Obtains the given range of capabilities from this activity to your activity. The selectors are
     * automatically chosen.
     *
     * @param crd the capabilities of this activity to delegate to your activity
     */
    void obtain(const KIF::CapRngDesc &crd);

    /**
     * Obtains the given range of capabilities from this activity to your activity at position <dest>.
     *
     * @param crd the capabilities of this activity to delegate to your activity
     * @param dest the destination in your activity
     */
    void obtain(const KIF::CapRngDesc &crd, capsel_t dest);

    /**
     * Revokes the given range of capabilities from this activity.
     *
     * @param crd the capabilities to revoke
     * @param delonly whether to revoke delegations only
     */
    void revoke(const KIF::CapRngDesc &crd, bool delonly = false);

    /**
     * Creates a new memory-gate for the memory region [addr..addr+size) of this activity's address
     * space with given permissions.
     *
     * @param act the activity
     * @param addr the address (page aligned)
     * @param size the memory size (page aligned)
     * @param perms the permissions (see MemGate::RWX)
     * @return the memory gate
     */
    MemGate get_mem(goff_t addr, size_t size, int perms);

    /**
     * Starts the activity, i.e., prepares the tile for execution and wakes it up.
     */
    void start();

    /**
     * Stops the activity, i.e., if it is running, the execution is stopped.
     */
    void stop();

    /**
     * Waits until the currently executing program on this activity is finished
     *
     * @return the exitcode
     */
    int wait();

    /**
     * Starts to wait until the currently executing program on this activity is finished, but tells
     * to kernel to notify us asynchronously via upcall.
     *
     * @param event the event for the upcall
     * @return 0 on success
     */
    int wait_async(event_t event);

    /**
     * Executes the given program with this activity.
     *
     * @param argc the number of arguments to pass to the program
     * @param argv the arguments to pass (argv[0] is the executable)
     */
    void exec(int argc, const char **argv);

    /**
     * Executes the program of Activity::self() (argv[0]) with this activity and calls the given
     * function instead of main.
     *
     * This has a few requirements/limitations:
     * 1. the current binary has to be stored in a file system
     * 2. this file system needs to be mounted, such that argv[0] is the current binary
     *
     * @param func the function to execute
     */
    void run(int (*func)());

private:
    void mark_caps_allocated(capsel_t sel, uint count) {
        _next_sel = Math::max(_next_sel, sel + count);
    }

    static void reset() noexcept;

    void obtain_mounts();
    void obtain_fds();

    void init_state();
    void init_fs();
    void do_exec(int argc, const char **argv, uintptr_t func_addr);
    void load_segment(ElfPh &pheader, char *buffer);
    void load(int argc, const char **argv, uintptr_t *entry, char *buffer, size_t *size);
    void clear_mem(MemGate &mem, char *buffer, size_t count, uintptr_t dest);
    size_t serialize_state(Env &senv, char *buffer, size_t offset);
    size_t store_arguments(char *buffer, int argc, const char **argv);

    uintptr_t get_entry();

    actid_t _id;
    Reference<class Tile> _tile;
    Reference<KMem> _kmem;
    capsel_t _next_sel;
    epid_t _eps_start;
    EPMng _epmng;
    Reference<Pager> _pager;
    std::unique_ptr<ResMng> _resmng;
    std::unique_ptr<MountTable> _ms;
    std::unique_ptr<FileTable> _fds;
    std::unique_ptr<FStream> _exec;
    unsigned char _data[DATA_SIZE];
    static Activity _self;
};

}
