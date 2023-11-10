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

#include <base/TCU.h>

#include <m3/tiles/Activity.h>

namespace m3 {

class FileTable;
class MountTable;

/**
 * Represents the own activity.
 */
class OwnActivity : public Activity {
    friend class Activity;

    static const size_t BUF_SIZE;
    static constexpr size_t DATA_SIZE = 256;

    explicit OwnActivity();

public:
    virtual ~OwnActivity();

    /**
     * Puts the own activity to sleep until the next message arrives
     */
    static void sleep() noexcept {
        sleep_for(TimeDuration::MAX);
    }

    /**
     * Puts the own activity to sleep until the next message arrives or <nanos> nanoseconds have
     * passed.
     */
    static void sleep_for(TimeDuration duration) noexcept {
        if(env()->shared || duration != TimeDuration::MAX)
            TMIF::wait(TCU::INVALID_EP, INVALID_IRQ, duration);
        else if(env()->platform != Platform::HW)
            TCU::get().wait_for_msg(TCU::INVALID_EP);
    }

    /**
     * Puts the own activity to sleep until the next message arrives on the given EP
     */
    static void wait_for_msg(epid_t ep) noexcept {
        if(env()->shared)
            TMIF::wait(ep, INVALID_IRQ, TimeDuration::MAX);
        else if(env()->platform != Platform::HW)
            TCU::get().wait_for_msg(ep);
    }

    /**
     * @return the resource manager
     */
    std::unique_ptr<ResMng> &resmng() noexcept {
        return _resmng;
    }

    /**
     * @return the mount table of this activity
     */
    std::unique_ptr<MountTable> &mounts() noexcept {
        return _ms;
    }

    /**
     * @return the files of this activity
     */
    std::unique_ptr<FileTable> &files() noexcept {
        return _fds;
    }

    /**
     * Returns an unmarshaller for the activity-local data.
     *
     * The source provides access to the activity-local data that has been transmitted to this
     * activity from its parent during Activity::run or Activity::exec.
     *
     * @return an unmarshaller to read from the activity-local data
     */
    Unmarshaller data_source() noexcept {
        return Unmarshaller(_data, sizeof(_data));
    }

    /**
     * @return the endpoint manager for this activity
     */
    EPMng &epmng() {
        return _epmng;
    }

private:
    void init_state();
    void init_fs();

    EPMng _epmng;
    std::unique_ptr<ResMng> _resmng;
    std::unique_ptr<MountTable> _ms;
    std::unique_ptr<FileTable> _fds;
    static OwnActivity _self;
};

}
