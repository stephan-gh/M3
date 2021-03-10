/*
 * Copyright (C) 2021, Tendsin Mende <tendsin.mende@mailbox.tu-dresden.de>
 * Copyright (C) 2017, Georg Kotheimer <georg.kotheimer@mailbox.tu-dresden.de>
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

#include <base/col/Treap.h>

#include <m3/com/SendGate.h>
#include <m3/netrs/Net.h>
#include <m3/netrs/NetChannel.h>
#include <m3/session/ClientSession.h>
#include <m3/vfs/GenericFile.h>

namespace m3 {

class NetworkManagerRs : public ClientSession {
private:
    // Thin wrapper around the NetElement so we dont destroy the NetData memory layout
    struct NetElement : public SListItem {
        NetElement(m3::net::NetData *new_el) {
            el = new_el;
        }
        m3::net::NetData *el;
    };

    struct RecvElement : public TreapNode<RecvElement, int32_t> {
        RecvElement(m3::net::NetData *first_element)
            : m3::TreapNode<RecvElement, int32_t>(first_element->sd), waiting_packages() {
            push(first_element);
        }
        /// Returns true if there is an element
        bool has_element() {
            return waiting_packages.length() > 0;
        }

        // Pops an element from the queue
        m3::net::NetData *pop_element() {
            NetElement *removed = waiting_packages.remove_first();
            if(removed != nullptr) {
                m3::net::NetData *dptr = removed->el;
                delete removed;
                return dptr;
            }
            else {
                return nullptr;
            }
        }

        void push(m3::net::NetData *element) {
            waiting_packages.append(new NetElement(element));
        }

        /// deletes all elements in the list
        void clear() {
            NetElement *pkg;
            while((pkg = waiting_packages.remove_first()) != nullptr) {
                // dealloc inner NetData, then delete self
                delete pkg->el;
                delete pkg;
            }
        }

        // Manages order of waiting packages
        SList<NetElement> waiting_packages;
    };

public:
    enum Operation {
        STAT     = GenericFile::STAT,
        SEEK     = GenericFile::SEEK,
        NEXT_IN  = GenericFile::NEXT_IN,
        NEXT_OUT = GenericFile::NEXT_OUT,
        COMMIT   = GenericFile::COMMIT,
        CLOSE    = GenericFile::CLOSE,
        CREATE,
        BIND,
        LISTEN,
        CONNECT,
        ACCEPT,
        // SEND, // provided by pipes
        // RECV, // provided by pipes
        COUNT,
        QUERY_STATE,
        TICK,
    };

    explicit NetworkManagerRs(const String &service);
    ~NetworkManagerRs();

    const SendGate &meta_gate() const noexcept {
        return _metagate;
    }

    // Creates a new socket on the service anr returns the descriptor. Returns -1 if failed.
    int32_t create(SocketType type, uint8_t protocol = 0);
    void bind(int32_t sd, IpAddr addr, uint16_t port);
    void listen(int32_t sd, IpAddr local_addr, uint16_t port);
    void connect(int32_t sd, IpAddr remote_addr, uint16_t remote_port, IpAddr local_addr,
                 uint16_t local_port);
    void close(int32_t sd);
    void as_file(int32_t sd, int mode, MemGate &mem, size_t memsize, fd_t &fd);
    void notify_drop(int32_t sd);
    void send(int32_t sd, IpAddr src_sddr, uint16_t src_port, IpAddr dst_addr, uint16_t dst_port,
              uint8_t *data, uint32_t data_length);
    /// Returns non empty packge if some package was in the queue, or an empty package if none was in the queue.
    m3::net::NetData recv(int32_t sd);
    SocketState get_state(int32_t sd);

private:
    void update_recv_queue();

    SendGate _metagate;
    NetChannel _channel;
    /// Keeps track of all received packges, keyed by their socket descriptor number.
    Treap<RecvElement> _receive_queue;
};

}
