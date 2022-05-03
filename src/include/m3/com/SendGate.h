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

#include <base/Errors.h>

#include <m3/com/Gate.h>
#include <m3/com/RecvGate.h>

namespace pci {
class ProxiedPciDevice;
}

namespace m3 {

class EnvUserBackend;
class SendGate;
class Syscalls;
class Activity;

/**
 * The optional arguments for SendGate::create()
 */
class SendGateArgs {
    friend class SendGate;

public:
    explicit SendGateArgs() noexcept;

    /**
     * The flags for the capability (default = 0)
     */
    SendGateArgs &flags(uint flags) noexcept {
        _flags = flags;
        return *this;
    }
    /**
     * The receive gate to use for replies (default = RecvGate::def())
     */
    SendGateArgs &reply_gate(RecvGate *reply_gate) noexcept {
        _replygate = reply_gate;
        return *this;
    }
    /**
     * The label for the SendGate (default = 0)
     */
    SendGateArgs &label(label_t label) noexcept {
        _label = label;
        return *this;
    }
    /**
     * The number of credits in bytes (default = UNLIMITED)
     */
    SendGateArgs &credits(uint credits) noexcept {
        _credits = credits;
        return *this;
    }
    /**
     * The selector to use (default = auto select)
     */
    SendGateArgs &sel(capsel_t sel) noexcept {
        _sel = sel;
        return *this;
    }

private:
    uint _flags;
    RecvGate *_replygate;
    label_t _label;
    uint _credits;
    capsel_t _sel;
};

/**
 * A SendGate is used to send messages to a RecvGate. To receive replies for the sent messages,
 * it has an associated RecvGate. You can either create a SendGate for a RecvGate and delegate it
 * to somebody else in order to allow him to send messages to this RecvGate. Or you can bind a
 * SendGate to a capability you've received from somebody else.
 */
class SendGate : public Gate {
    friend class Syscalls;
    friend class EnvUserBackend;
    friend class Pager;
    friend class AladdinAccel;
    friend class InDirAccel;
    friend class StreamAccel;
    friend class pci::ProxiedPciDevice;

    explicit SendGate(capsel_t cap, uint capflags, RecvGate *replygate,
                      epid_t ep = UNBOUND) noexcept
        : Gate(SEND_GATE, cap, capflags, ep),
          _replygate(replygate == nullptr ? &RecvGate::def() : replygate) {
    }

public:
    static const uint UNLIMITED = KIF::UNLIM_CREDITS;

    /**
     * Creates a new send gate for the given receive gate.
     *
     * @param rgate the destination receive gate
     * @param args additional arguments
     * @return the send gate
     */
    static SendGate create(RecvGate *rgate, const SendGateArgs &args = SendGateArgs());

    /**
     * Creates the send gate with given name as defined in the application's configuration.
     *
     * @param name the name in the configuration file
     * @param replygate the receive gate to which the replies should be sent
     * @return the send gate
     */
    static SendGate create_named(const char *name, RecvGate *replygate = nullptr);

    /**
     * Binds this send gate to the given capability. Typically, received from somebody else.
     *
     * @param sel the capability selector
     * @param replygate the receive gate to which the replies should be sent
     * @return the send gate
     */
    static SendGate bind(capsel_t sel, RecvGate *replygate = nullptr) noexcept {
        return SendGate(sel, ObjCap::KEEP_CAP, replygate);
    }

    SendGate(SendGate &&g) noexcept : Gate(std::move(g)), _replygate(g._replygate) {
    }

    /**
     * @return the gate to receive the replies from when sending a message over this gate
     */
    RecvGate *reply_gate() noexcept {
        return _replygate;
    }
    /**
     * Sets the receive gate to receive replies on.
     *
     * @param rgate the new receive gate
     */
    void reply_gate(RecvGate *rgate) noexcept {
        _replygate = rgate;
    }

    /**
     * @return true if this SendGate can potentially send a message
     */
    bool can_send() const noexcept {
        return !ep() || TCU::get().credits(ep()->id()) > 0;
    }

    /**
     * @return the currently available credits
     */
    uint credits();

    /**
     * Sends <msg> to the associated RecvGate.
     *
     * @param msg the message to send
     * @param reply_label the reply label to set
     */
    void send(const MsgBuf &msg, label_t reply_label = 0);

    /**
     * Sends <msg> with <len> bytes to the associated RecvGate and returns the error code on
     * failure. Assumes that <msg>:<len> is properly aligned and does not contain a page boundary.
     *
     * @param msg the message to send
     * @param len the length of the message
     * @param reply_label the reply label to set
     * @return the error code if failed
     */
    void send_aligned(const void *msg, size_t len, label_t reply_label = 0);

    /**
     * Tries to send <msg> to the associated RecvGate and returns the error code on failure.
     *
     * @param msg the message to send
     * @param reply_label the reply label to set
     * @return the error code if failed
     */
    Errors::Code try_send(const MsgBuf &msg, label_t reply_label = 0);

    /**
     * Tries to send <msg> with <len> bytes to the associated RecvGate and returns the error code on
     * failure. Assumes that <msg>:<len> is properly aligned and does not contain a page boundary.
     *
     * @param msg the message to send
     * @param len the length of the message
     * @param reply_label the reply label to set
     * @return the error code if failed
     */
    Errors::Code try_send_aligned(const void *msg, size_t len, label_t reply_label = 0);

    /**
     * Sends <msg> to the associated RecvGate and receives the reply from the set reply gate.
     *
     * @param msg the message to send
     * @return the received reply
     */
    const TCU::Message *call(const MsgBuf &msg);

private:
    RecvGate *_replygate;
};

inline SendGateArgs::SendGateArgs() noexcept
    : _flags(),
      _replygate(),
      _label(),
      _credits(SendGate::UNLIMITED),
      _sel(ObjCap::INVALID) {
}

}
