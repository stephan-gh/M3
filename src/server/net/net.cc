/*
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

#include <base/Common.h>
#include <base/log/Services.h>
#include <base/util/Profile.h>
#include <base/Panic.h>

#include <m3/net/Net.h>
#include <m3/server/RequestHandler.h>
#include <m3/server/Server.h>
#include <m3/stream/Standard.h>
#include <m3/vfs/VFS.h>
#include <pci/Device.h>
#include <thread/ThreadManager.h>

#include "driver/driver.h"

#include "lwipopts.h"
#include "lwip/sys.h"
#include "lwip/init.h"
#include "lwip/netif.h"
#include "lwip/ip_addr.h"
#include "lwip/pbuf.h"
#include "lwip/prot/ethernet.h"
#include "lwip/etharp.h"
#include "lwip/timeouts.h"

#include "sess/SocketSession.h"

#include <assert.h>
#include <m3/session/NetworkManager.h>
#include <m3/session/ServerSession.h>
#include <queue>
#include <cstring>
#include <memory>

using namespace net;
using namespace m3;

class NMRequestHandler;
using net_reqh_base_t = RequestHandler<
    NMRequestHandler, NetworkManager::Operation, NetworkManager::COUNT, NMSession
>;

static Server<NMRequestHandler> *srv;

class NMRequestHandler: public net_reqh_base_t {
public:
    explicit NMRequestHandler(WorkLoop *wl, NetDriver *driver)
        : net_reqh_base_t(),
          _wl(wl),
          _driver(driver),
          _rgate(RecvGate::create(nextlog2<32 * NMSession::MSG_SIZE>::val, nextlog2<NMSession::MSG_SIZE>::val)) {
        add_operation(NetworkManager::CREATE, &NMRequestHandler::create);
        add_operation(NetworkManager::BIND, &NMRequestHandler::bind);
        add_operation(NetworkManager::LISTEN, &NMRequestHandler::listen);
        add_operation(NetworkManager::CONNECT, &NMRequestHandler::connect);
        add_operation(NetworkManager::CLOSE, &NMRequestHandler::close);
        add_operation(NetworkManager::STAT, &NMRequestHandler::stat);
        add_operation(NetworkManager::SEEK, &NMRequestHandler::seek);
        add_operation(NetworkManager::NEXT_IN, &NMRequestHandler::next_in);
        add_operation(NetworkManager::NEXT_OUT, &NMRequestHandler::next_out);
        add_operation(NetworkManager::COMMIT, &NMRequestHandler::commit);

        using std::placeholders::_1;
        _rgate.start(wl, std::bind(&NMRequestHandler::handle_message, this, _1));
    }

    virtual Errors::Code open(NMSession **sess, capsel_t srv_sel, const String &) override {
        *sess = new SocketSession(_wl, srv_sel, _rgate);
        _sessions.append(*sess);
        return Errors::NONE;
    }

    virtual Errors::Code obtain(NMSession *sess, KIF::Service::ExchangeData &data) override {
        return sess->obtain(srv->sel(), data);
    }

    virtual Errors::Code delegate(NMSession *sess, KIF::Service::ExchangeData &data) override {
        return sess->delegate(data);
    }

    virtual Errors::Code close(NMSession *sess) override {
        if(sess->type() == NMSession::SOCKET)
            _sessions.remove(sess);
        delete sess;
        _rgate.drop_msgs_with(reinterpret_cast<label_t>(sess));
        return Errors::NONE;
    }

    virtual void shutdown() override {
        // delete sessions to remove items from workloop etc.
        for(auto it = _sessions.begin(); it != _sessions.end(); ) {
            auto old = it++;
            delete &*old;
        }

        _driver->stop();
        _rgate.stop();
    }

    void create(GateIStream &is) {
        NMSession *sess = is.label<NMSession*>();
        sess->create(is);
    }

    void bind(GateIStream &is) {
        NMSession *sess = is.label<NMSession*>();
        sess->bind(is);
    }

    void listen(GateIStream &is) {
        NMSession *sess = is.label<NMSession*>();
        sess->listen(is);
    }

    void connect(GateIStream &is) {
        NMSession *sess = is.label<NMSession*>();
        sess->connect(is);
    }

    void close(GateIStream &is) {
        NMSession *sess = is.label<NMSession*>();
        sess->close(is);
        // we don't get the close call from our resource manager for files (child sessions)
        if(sess->type() == NMSession::FILE)
            close(sess);
    }

    void next_in(GateIStream &is) {
        NMSession *sess = is.label<NMSession*>();
        sess->next_in(is);
    }

    void next_out(GateIStream &is) {
        NMSession *sess = is.label<NMSession*>();
        sess->next_out(is);
    }

    void commit(GateIStream &is) {
        NMSession *sess = is.label<NMSession*>();
        sess->commit(is);
    }

    void seek(GateIStream &is) {
        NMSession *sess = is.label<NMSession*>();
        sess->seek(is);
    }

    void stat(GateIStream &is) {
        NMSession *sess = is.label<NMSession*>();
        sess->stat(is);
    }

private:
    WorkLoop *_wl;
    NetDriver *_driver;
    RecvGate _rgate;
    SList<NMSession> _sessions;
};

static std::queue<struct pbuf*> recvQueue;

static bool eth_alloc_callback(void *&pkt, void *&buf, size_t &buf_size, size_t size) {
    /* Allocate pbuf from pool */
    struct pbuf *p = pbuf_alloc(PBUF_RAW, size, PBUF_POOL);

    if(p != NULL) {
        pkt = p;
        buf = p->payload;
        buf_size = p->len;
        return true;
    } else
        return false;
}

static void eth_next_buf_callback(void *&pkt, void *&buf, size_t &buf_size) {
    struct pbuf *p = static_cast<struct pbuf *>(pkt)->next;
    pkt = p;
    buf = p ? p->payload : 0;
    buf_size = p ? p->len : 0;
}

static void eth_recv_callback(void *pkt) {
    struct pbuf *p = static_cast<pbuf *>(pkt);

    /* Put in a queue which is processed in main loop */
    // TODO: Get rid of the queue or at least limit it in size.
    recvQueue.push(p);
}

static err_t netif_output(struct netif *netif, struct pbuf *p) {
    LINK_STATS_INC(link.xmit);

    SLOG(NET, "netif_output with size " << p->len);

    if(p->next) {
        // TODO: Use copy callback instead of scratch buffer
        SLOG(NET, "netif_output: Using scratch buffer for pbuf chain.");
        uint8_t *pkt = (uint8_t*)malloc(p->tot_len);
        if(!pkt) {
            SLOG(NET, "Not enough memory to read packet");
            return ERR_MEM;
        }

        pbuf_copy_partial(p, pkt, p->tot_len, 0);
        NetDriver *driver = static_cast<NetDriver*>(netif->state);
        if(!driver->send(pkt, p->tot_len)) {
            free(pkt);
            SLOG(NET, "netif_output failed!");
            return ERR_IF;
        }

        free(pkt);
    } else {
        NetDriver *driver = static_cast<NetDriver*>(netif->state);
        if(!driver->send(p->payload, p->tot_len)) {
            SLOG(NET, "netif_output failed!");
            return ERR_IF;
        }
    }

    return ERR_OK;
}

static void netif_status_callback(struct netif *netif) {
    SLOG(NET, "netif status changed " << ipaddr_ntoa(netif_ip4_addr(netif))
        << " to " << ((netif->flags & NETIF_FLAG_UP) ? "up" : "down"));
}

static err_t netif_init(struct netif *netif) {
    NetDriver *driver = static_cast<NetDriver*>(netif->state);

    netif->linkoutput = netif_output;
    netif->output = etharp_output;
    // netif->output_ip6 = ethip6_output;
    netif->mtu = 1500;
    netif->flags = NETIF_FLAG_BROADCAST | NETIF_FLAG_ETHARP | NETIF_FLAG_ETHERNET |
                   NETIF_FLAG_IGMP | NETIF_FLAG_MLD6;

    m3::net::MAC mac = driver->readMAC();
    static_assert(m3::net::MAC::LEN == sizeof(netif->hwaddr), "mac address size mismatch");
    SMEMCPY(netif->hwaddr, mac.bytes(), m3::net::MAC::LEN);
    netif->hwaddr_len = sizeof(netif->hwaddr);

    return ERR_OK;
}

static bool link_state_changed(struct netif *netif) {
    return static_cast<NetDriver *>(netif->state)->linkStateChanged();
}

static bool link_is_up(struct netif *netif) {
    return static_cast<NetDriver *>(netif->state)->linkIsUp();
}

int main(int argc, char **argv) {
    if(argc != 4)
        exitmsg("Usage: " << argv[0] << " <name> <ip address> <netmask>");

    ip_addr_t ip;
    if(!ipaddr_aton(argv[2], &ip))
        exitmsg(argv[2] << " is not a well formed ip address.");
    else
        SLOG(NET, "ip: " << ipaddr_ntoa(&ip));

    ip_addr_t netmask;
    if(!ipaddr_aton(argv[3], &netmask))
        exitmsg(argv[3] << " is not a well formed netmask.");
    else SLOG(NET, "netmask: " << ipaddr_ntoa(&netmask));

    struct netif netif;

    WorkLoop wl;

    NetDriver *driver = NetDriver::create(argv[1], &wl, eth_alloc_callback, eth_next_buf_callback,
                                          eth_recv_callback);

    lwip_init();

    netif_add(&netif, &ip, &netmask, IP4_ADDR_ANY, driver, netif_init, netif_input);
    netif.name[0] = 'e';
    netif.name[1] = '0';
    // netif_create_ip6_linklocal_address(&netif, 1);
    // netif.ip6_autoconfig_enabled = 1;
    netif_set_status_callback(&netif, netif_status_callback);
    netif_set_default(&netif);
    netif_set_up(&netif);

    /* Start DHCP */
    // dhcp_start(&netif );
    srv = new Server<NMRequestHandler>(argv[1], &wl, new NMRequestHandler(&wl, driver));

    while(wl.has_items()) {
        /* Check link state, e.g. via MDIO communication with PHY */
        if(link_state_changed(&netif)) {
            if(link_is_up(&netif))
                netif_set_link_up(&netif);
            else
                netif_set_link_down(&netif);
        }

        /* Check for received frames, feed them to lwIP */
        size_t maxReceiveCount = SocketSession::MAX_SEND_RECEIVE_BATCH_SIZE;
        while(!recvQueue.empty() && maxReceiveCount--) {
            struct pbuf *p = recvQueue.front();
            recvQueue.pop();
            LINK_STATS_INC(link.recv);

            err_t err = netif.input(p, &netif);
            if(err != ERR_OK) {
                SLOG(NET, "netif.input() failed with error " << err << ", dropping packet!");
                pbuf_free(p);
            }
        }

        /* Cyclic lwIP timers check */
        sys_check_timeouts();

        // Hack: run the workloop manually
        // - interrupt receive gate
        wl.tick();

        // TODO: Consider FileSession::handle_..., More?
        // Sleep according to sys_timeouts_sleeptime() if there is nothing to do.
        if(recvQueue.empty()) {
            if(auto sleep_ms = sys_timeouts_sleeptime()) {
                cycles_t sleep_time = sleep_ms * (m3::DTU::get().clock() / 1000);
                cycles_t start = m3::DTU::get().tsc();
                SLOG(NET_ALL, "@" << start << " try_sleep: " << sleep_time << " cycles"
                    << " (" << sleep_ms << " ms)");

                DTU::get().try_sleep(false, sleep_time);

                cycles_t stop = m3::DTU::get().tsc();
                SLOG(NET_ALL, "@" << stop << " wakeup: " << stop - start << " cycles"
                    << " (" << ((stop - start) * 1000 / m3::DTU::get().clock()) << " ms)");
            }
        }
    }

    delete srv;
    delete driver;

    return 0;
}
