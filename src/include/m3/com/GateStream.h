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

#include <m3/Exception.h>
#include <m3/com/Marshalling.h>
#include <m3/com/MemGate.h>
#include <m3/com/RecvGate.h>
#include <m3/com/SendGate.h>

#include <alloca.h>

namespace m3 {

/**
 * The gate stream classes provide an easy abstraction to marshall or unmarshall data when
 * communicating between activities. Therefore, if you want to combine multiple values into a single
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
 * The gate stream to marshall values into a message and send it over an endpoint. Thus, it
 * "outputs" values into a message.
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
 * An implementation of GateOStream that uses MsgBuf to store the message.
 */
class MsgGateOStream : public GateOStream {
public:
    explicit MsgGateOStream() noexcept : GateOStream(0, MsgBuf::MAX_MSG_SIZE), _msg() {
        _bytes = reinterpret_cast<unsigned char *>(_msg.bytes());
    }
    MsgGateOStream(const MsgGateOStream &os) noexcept : GateOStream(os), _msg(os._msg) {
    }
    MsgGateOStream &operator=(const MsgGateOStream &os) noexcept {
        GateOStream::operator=(os);
        if(&os != this)
            _msg = os._msg;
        return *this;
    }

    MsgBuf &finish() noexcept {
        _msg.set_size(total());
        return _msg;
    }

private:
    MsgBuf _msg;
};

/**
 * An output stream for the exchange arguments.
 */
class ExchangeOStream : public Marshaller {
public:
    explicit ExchangeOStream(KIF::ExchangeArgs &args) noexcept
        : Marshaller(args.data, sizeof(args.data)) {
    }
    ExchangeOStream(const ExchangeOStream &) = delete;
    ExchangeOStream &operator=(const ExchangeOStream &) = delete;

    using Marshaller::put;
};

/**
 * An input stream for the exchange arguments.
 */
class ExchangeIStream : public Unmarshaller {
public:
    explicit ExchangeIStream(const KIF::ExchangeArgs &args) noexcept
        : Unmarshaller(args.data, args.bytes) {
    }
    ExchangeIStream(const ExchangeIStream &) = delete;
    ExchangeIStream &operator=(const ExchangeIStream &) = delete;
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
    explicit GateIStream(RecvGate &rgate, const TCU::Message *msg) noexcept
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
    const TCU::Message &message() const noexcept {
        return *_msg;
    }
    /**
     * @return the label of the message
     */
    template<typename T>
    T label() const noexcept {
        return (T) static_cast<word_t>(_msg->label);
    }

    /**
     * Pulls an error code from this GateIStream and throws an exception if it is not Errors::NONE.
     */
    void pull_result() {
        Errors::Code res;
        *this >> res;
        if(res != Errors::SUCCESS)
            throw Exception(res);
    }

    /**
     * Replies the given message to this one
     *
     * @param reply the message
     */
    void reply(const MsgBuf &reply) {
        reply_aligned(reply.bytes(), reply.size());
    }

    /**
     * Replies the given message to this one, assuming that the reply is properly aligned. The
     * message address needs to be 16-byte aligned and the message cannot contain a page boundary.
     *
     * @param reply the message
     * @param len the length of the reply
     */
    void reply_aligned(const void *reply, size_t len) {
        _rgate->reply_aligned(reply, len, _msg);
        _ack = false;
    }

    /**
     * Disables acknowledgement of the message. That is, it will be marked as read, but you have
     * to ack the message on your own via RecvGate::ack_msg().
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
            _rgate->ack_msg(_msg);
            _ack = false;
        }
    }

private:
    bool _ack;
    RecvGate *_rgate;
    const TCU::Message *_msg;
};

inline void GateOStream::put(const GateIStream &is) noexcept {
    assert(fits(_bytecount, is.remaining()));
    memcpy(const_cast<unsigned char *>(bytes()) + _bytecount, is.buffer() + is.pos(),
           is.remaining());
    _bytecount += is.remaining();
}

static inline void reply_error(GateIStream &is, m3::Errors::Code error) {
    MsgBuf reply;
    auto &reply_data = reply.cast<KIF::DefaultReply>();
    reply_data.error = static_cast<xfer_t>(error);
    is.reply(reply);
}

/**
 * All these methods send the given message; either over <gate> or as an reply to the first not
 * acknowledged message in <gate> or as a reply on a GateIStream.
 *
 * @param gate the gate to send to
 * @param msg the message
 */
static inline void send_msg(SendGate &gate, const MsgBuf &msg) {
    gate.send(msg);
}
static inline void reply_msg(GateIStream &is, const MsgBuf &msg) {
    is.reply(msg);
}

/**
 * Creates a StaticGateOStream for the given arguments.
 *
 * @return the stream
 */
template<typename... Args>
static inline MsgGateOStream create_vmsg(const Args &...args) noexcept {
    static_assert(ostreamsize<Args...>() <= MsgBuf::MAX_MSG_SIZE,
                  "Arguments too large for message");
    MsgGateOStream os;
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
static inline void send_vmsg(SendGate &gate, const Args &...args) {
    auto msg = create_vmsg(args...);
    gate.send(msg.finish());
}
template<typename... Args>
static inline void reply_vmsg(GateIStream &is, const Args &...args) {
    auto reply = create_vmsg(args...);
    is.reply(reply.finish());
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
static inline void write_vmsg(MemGate &gate, size_t offset, const Args &...args) {
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
    const TCU::Message *msg = rgate.receive(nullptr);
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
static inline GateIStream receive_vmsg(RecvGate &rgate, Args &...args) {
    const TCU::Message *msg = rgate.receive(nullptr);
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
    const TCU::Message *msg = gate.reply_gate()->receive(&gate);
    return GateIStream(*gate.reply_gate(), msg);
}

/**
 * Convenience methods that combine send_msg()/send_vmsg() and receive_msg().
 */
static inline GateIStream send_receive_msg(SendGate &gate, const MsgBuf &msg) {
    const TCU::Message *reply = gate.call(msg);
    return GateIStream(*gate.reply_gate(), reply);
}
template<typename... Args>
static inline GateIStream send_receive_vmsg(SendGate &gate, const Args &...args) {
    auto msg = create_vmsg(args...);
    const TCU::Message *reply = gate.call(msg.finish());
    return GateIStream(*gate.reply_gate(), reply);
}

}
