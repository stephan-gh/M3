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
#include <base/KIF.h>

#include <m3/cap/ObjCap.h>

#include <string_view>
#include <utility>

namespace m3 {

class Activity;

/**
 * A client session represents a connection between client and the server for the client. Over the
 * session, capabilities can be exchanged, e.g. to delegate a SendGate from server to client in
 * order to let the client send messages to the server.
 *
 * At construction, the server receives an OPEN event, that allows him to associate information with
 * this session. At destruction, the server receives a CLOSE event to perform cleanup.
 */
class ClientSession : public ObjCap {
public:
    /**
     * Opens a session at service <name>.
     *
     * @param name the service name
     * @param sel the desired selector
     */
    explicit ClientSession(const std::string_view &name, capsel_t sel = ObjCap::INVALID)
        : ObjCap(SESSION),
          _close(true) {
        open(name, sel);
    }

    /**
     * Attaches this object to the given session
     *
     * @param sel the capability selector of the session
     * @param flags whether capabilitly/selector should be kept on destruction or not
     */
    explicit ClientSession(capsel_t sel, uint flags = ObjCap::KEEP_CAP) noexcept
        : ObjCap(SESSION, sel, flags),
          _close(false) {
    }

    ClientSession(ClientSession &&s) noexcept : ObjCap(std::move(s)), _close(s._close) {
    }

    ~ClientSession();

    /**
     * Creates a connection for requests to the server for given activity
     *
     * The method uses the Connect operation to obtain a SendGate from the server that can be used
     * afterwards to send requests to the server.
     *
     * @return the obtained SendGate
     */
    SendGate connect();

    /**
     * Creates a connection for requests to the server for given activity
     *
     * The method uses the Connect operation to obtain a SendGate from the server that can be used
     * afterwards to send requests to the server. The SendGate will be obtained for the given
     * activity and bound to the given selector.
     *
     * @return the used selector (<sel>)
     */
    capsel_t connect_for(Activity &act, capsel_t sel);

    /**
     * Delegates the given object capability to the server.
     *
     * @param sel the capability
     */
    void delegate_obj(capsel_t sel) {
        KIF::CapRngDesc crd(KIF::CapRngDesc::OBJ, sel, 1);
        delegate(crd);
    }

    /**
     * Delegates the given capability range to the server with additional arguments and puts the
     * arguments from the server again into argcount and args.
     *
     * @param caps the capabilities
     * @param args the arguments to pass to the server
     */
    void delegate(const KIF::CapRngDesc &caps, KIF::ExchangeArgs *args = nullptr);

    /**
     * Delegates the given capability range of <act> to the server with additional arguments and
     * puts the arguments from the server again into argcount and args.
     *
     * @param act the act to do the delegate for
     * @param caps the capabilities
     * @param args the arguments to pass to the server
     */
    void delegate_for(Activity &act, const KIF::CapRngDesc &caps,
                      KIF::ExchangeArgs *args = nullptr);

    /**
     * Obtains up to <count> capabilities from the server with additional arguments and puts the
     * arguments from the server again into argcount and args.
     *
     * @param count the number of capabilities
     * @param args the arguments to pass to the server
     * @return the received capabilities
     */
    KIF::CapRngDesc obtain(uint count, KIF::ExchangeArgs *args = nullptr);

    /**
     * Obtains up to <count> capabilities from the server for <act> with additional arguments and
     * puts the arguments from the server again into argcount and args.
     *
     * @param act the act to do the obtain for
     * @param count the number of capabilities
     * @param args the arguments to pass to the server
     * @return the received capabilities
     */
    KIF::CapRngDesc obtain_for(Activity &act, uint count, KIF::ExchangeArgs *args = nullptr);

    /**
     * Obtains up to <crd>.count() capabilities from the server for <act> with additional arguments
     * and puts the arguments from the server again into argcount and args.
     *
     * @param act the act to do the obtain for
     * @param crd the selectors to use
     * @param argcount the number of arguments
     * @param args the arguments to pass to the server
     */
    void obtain_for(Activity &act, const KIF::CapRngDesc &crd, KIF::ExchangeArgs *args = nullptr);

private:
    void open(const std::string_view &name, capsel_t sel);

    bool _close;
};

}
