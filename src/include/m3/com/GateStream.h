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

#include <m3/com/Marshalling.h>
#include <m3/com/SendGate.h>
#include <m3/com/MemGate.h>
#include <m3/com/RecvGate.h>
#include <m3/DTUIf.h>
#include <m3/Exception.h>

#include <alloca.h>

namespace m3 {

/**
 * The gate stream classes provide an easy abstraction to marshall or unmarshall data when
 * communicating between VPEs. Therefore, if you want to combine multiple values into a single
 * message or extract multiple values from a message, this is the abstraction you might want to use.
 * If you already have the data to send, you should directly use the send method of SendGate. If
 * you don't want to extract values from a message but directly access the message, use the
 * data-field of the message you received.
 *
 * All classes work with (variadic) templates and are thus type-safe. Of course, that does not
 * relieve you from taking care that sender and receiver agree on the types of the values that are
 * exchanged via messaging.
 */

/**
 * The gate stream to marshall values into a message and send it over an endpoint. Thus, it "outputs"
 * values into a message.
 */
class GateOStream : public Marshaller {
public:
    explicit GateOStream(unsigned char *bytes, size_t total) noexcept : Marshaller(bytes, total) {
    }
    GateOStream(const GateOStream &) = default;
    GateOStream &operator=(const GateOStream &) = default;

    using Marshaller::put;

    /**
     * Puts all remaining items (the ones that haven't been read yet) of <is> into this GateOStream.
     *
     * @param is the GateIStream
     * @return *this
     */
    void put(const GateIStream &is) noexcept;
};

/**
 * An implementation of GateOStream that hosts the message as a member. E.g. you can put an object
 * of this class on the stack, which would host the message on the stack.
 * In most cases, you don't want to use this class yourself, but the free standing convenience
 * functions below that automatically determine <SIZE>.
 *
 * @param SIZE the max. size of the message
 */
template<size_t SIZE>
class StaticGateOStream : public GateOStream {
public:
    explicit StaticGateOStream() noexcept : GateOStream(_bytes, SIZE) {
    }
    template<size_t SRCSIZE>
    StaticGateOStream(const StaticGateOStream<SRCSIZE> &os) noexcept : GateOStream(os) {
        static_assert(SIZE >= SRCSIZE, "Incompatible sizes");
        memcpy(_bytes, os._bytes, sizeof(os._bytes));
    }
    template<size_t SRCSIZE>
    StaticGateOStream &operator=(const StaticGateOStream<SRCSIZE> &os) noexcept {
        static_assert(SIZE >= SRCSIZE, "Incompatible sizes");
        GateOStream::operator=(os);
        if(&os != this)
            memcpy(_bytes, os._bytes, sizeof(os._bytes));
        return *this;
    }

private:
    unsigned char _bytes[SIZE];
};

/**
 * An implementation of GateOStream that hosts the message on the stack by using alloca.
 */
class AutoGateOStream : public GateOStream {
public:
    ALWAYS_INLINE explicit AutoGateOStream(size_t size) noexcept
        : GateOStream(static_cast<unsigned char*>(alloca(size)), size) {
    }

    AutoGateOStream(AutoGateOStream &&os) noexcept : GateOStream(os) {
    }

    /**
     * Claim the ownership of the data from this class. Thus, it will not free it.
     */
    void claim() noexcept {
        this->_bytes = nullptr;
    }
};

/**
 * The gate stream to unmarshall values from a message. Thus, it "inputs" values from a message
 * into variables.
 */
class GateIStream : public Unmarshaller {
public:
    /**
     * Creates an object for the given message from <rgate>.
     *
     * @param rgate the receive gate
     */
    explicit GateIStream(RecvGate &rgate, const DTU::Message *msg) noexcept
        : Unmarshaller(msg->data, msg->length),
          _ack(true),
          _rgate(&rgate),
          _msg(msg) {
    }

    // don't do the ack twice. thus, copies never ack.
    GateIStream(const GateIStream &is) noexcept
        : Unmarshaller(is),
          _ack(),
          _rgate(is._rgate),
          _msg(is._msg) {
    }
    GateIStream &operator=(const GateIStream &is) noexcept {
        if(this != &is) {
            Unmarshaller::operator=(is);
            _ack = false;
            _rgate = is._rgate;
            _msg = is._msg;
        }
        return *this;
    }
    GateIStream &operator=(GateIStream &&is) noexcept {
        if(this != &is) {
            Unmarshaller::operator=(is);
            _ack = is._ack;
            _rgate = is._rgate;
            _msg = is._msg;
            is._ack = 0;
        }
        return *this;
    }
    GateIStream(GateIStream &&is) noexcept
        : Unmarshaller(std::move(is)),
          _ack(is._ack),
          _rgate(is._rgate),
          _msg(is._msg) {
        is._ack = 0;
    }
    ~GateIStream() {
        finish();
    }

    /**
     * @return the receive gate
     */
    RecvGate &rgate() noexcept {
        return *_rgate;
    }
    /**
     * @return the message (header + payload)
     */
    const DTU::Message &message() const noexcept {
        return *_msg;
    }
    /**
     * @return the label of the message
     */
    template<typename T>
    T label() const noexcept {
        return (T)_msg->label;
    }

    /**
     * Pulls an error code from this GateIStream and throws an exception if it is not Errors::NONE.
     */
    void pull_result() {
        Errors::Code res;
        *this >> res;
        if(res != Errors::NONE)
            throw Exception(res);
    }

    /**
     * Replies the message constructed by <os> to this message
     *
     * @param os the GateOStream hosting the message to reply
     */
    void reply(const GateOStream &os) {
        reply(os.bytes(), os.total());
    }
    /**
     * Replies the given message to this one
     *
     * @param data the message data
     * @param len the length of the message
     */
    void reply(const void *data, size_t len) {
        _rgate->reply(data, len, _msg);
        // it's already acked
        _ack = false;
    }

    /**
     * Disables acknowledgement of the message. That is, it will be marked as read, but you have
     * to ack the message on your own via RecvGate::mark_read().
     */
    void claim() noexcept {
        _ack = false;
    }

    /**
     * Finishes this message, i.e. moves the read-position in the ringbuffer forward. If
     * acknowledgement has not been disabled (see claim), it will be acked.
     */
    void finish() noexcept {
        if(_ack) {
            _rgate->mark_read(_msg);
            _ack = false;
        }
    }

private:
    bool _ack;
    RecvGate *_rgate;
    const DTU::Message *_msg;
};

inline void GateOStream::put(const GateIStream &is) noexcept {
    assert(fits(_bytecount, is.remaining()));
    memcpy(const_cast<unsigned char*>(bytes()) + _bytecount, is.buffer() + is.pos(), is.remaining());
    _bytecount += is.remaining();
}

static inline void reply_error(GateIStream &is, m3::Errors::Code error) {
    KIF::DefaultReply reply;
    reply.error = static_cast<xfer_t>(error);
    is.reply(&reply, sizeof(reply));
}

/**
 * All these methods send the given data; either over <gate> or as an reply to the first not
 * acknowledged message in <gate> or as a reply on a GateIStream.
 *
 * @param gate the gate to send to
 * @param data the message data
 * @param len the message length
 */
static inline void send_msg(SendGate &gate, const void *data, size_t len) {
    gate.send(data, len);
}
static inline void reply_msg(GateIStream &is, const void *data, size_t len) {
    is.reply(data, len);
}

/**
 * Creates a StaticGateOStream for the given arguments.
 *
 * @return the stream
 */
template<typename ... Args>
static inline auto create_vmsg(const Args& ... args) noexcept -> StaticGateOStream<ostreamsize<Args...>()> {
    StaticGateOStream<ostreamsize<Args...>()> os;
    os.vput(args...);
    return os;
}

/**
 * All these methods put a message of the appropriate size, depending on the types of <args>, on the
 * stack, copies the values into it and sends it; either over <gate> or as an reply to the first not
 * acknowledged message in <gate> or as a reply on a GateIStream.
 *
 * @param gate the gate to send to
 * @param args the arguments to put into the message
 */
template<typename... Args>
static inline void send_vmsg(SendGate &gate, const Args &... args) {
    auto msg = create_vmsg(args...);
    gate.send(msg.bytes(), msg.total());
}
template<typename... Args>
static inline void reply_vmsg(GateIStream &is, const Args &... args) {
    is.reply(create_vmsg(args...));
}

/**
 * Puts a message of the appropriate size, depending on the types of <args>, on the
 * stack, copies the values into it and writes it to <gate> at <offset>.
 *
 * @param gate the memory gate
 * @param offset the offset to write to
 * @param args the arguments to marshall
 */
template<typename... Args>
static inline void write_vmsg(MemGate &gate, size_t offset, const Args &... args) {
    auto os = create_vmsg(args...);
    gate.write(os.bytes(), os.total(), offset);
}

/**
 * Receives a message from <gate> and returns an GateIStream to unmarshall the message. Note that
 * the GateIStream object acknowledges the message on destruction.
 *
 * @param rgate the gate to receive the message from
 * @return the GateIStream
 */
static inline GateIStream receive_msg(RecvGate &rgate) {
    const DTU::Message *msg = rgate.receive(nullptr);
    return GateIStream(rgate, msg);
}
/**
 * Receives a message from <gate> and unmarshalls the message into <args>. Note that
 * the GateIStream object acknowledges the message on destruction.
 *
 * @param rgate the gate to receive the message from
 * @param args the arguments to unmarshall to
 * @return the GateIStream, e.g. to read further values or to reply
 */
template<typename... Args>
static inline GateIStream receive_vmsg(RecvGate &rgate, Args &... args) {
    const DTU::Message *msg = rgate.receive(nullptr);
    GateIStream is(rgate, msg);
    is.vpull(args...);
    return is;
}

/**
 * Receives the reply for a message sent over <gate> and returns an GateIStream to unmarshall the
 * message. Note that the GateIStream object acknowledges the message on destruction.
 * The difference to receive_v?msg() is, that it
 *
 * @param gate the gate to receive the message from
 * @return the GateIStream
 */
static inline GateIStream receive_reply(SendGate &gate) {
    const DTU::Message *msg = gate.reply_gate()->receive(&gate);
    return GateIStream(*gate.reply_gate(), msg);
}

/**
 * Convenience methods that combine send_msg()/send_vmsg() and receive_msg().
 */
static inline GateIStream send_receive_msg(SendGate &gate, const void *data, size_t len) {
    const DTU::Message *reply = gate.call(data, len);
    return GateIStream(*gate.reply_gate(), reply);
}
template<typename... Args>
static inline GateIStream send_receive_vmsg(SendGate &gate, const Args &... args) {
    auto msg = create_vmsg(args...);
    const DTU::Message *reply = gate.call(msg.bytes(), msg.total());
    return GateIStream(*gate.reply_gate(), reply);
}

}
