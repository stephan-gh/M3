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

#ifndef LWIP_LWIPOPTS_H
#define LWIP_LWIPOPTS_H

#define NO_SYS 1
#define LWIP_CALLBACK_API 1
#define LWIP_PROVIDE_ERRNO 1

// Use malloc/free/realloc provided by your C-library instead of the lwip internal allocator.
#define MEM_LIBC_MALLOC 1
#define PBUF_POOL_SIZE 128

// There is no preemption in m3, so we can disable preemption protection.
#define SYS_LIGHTWEIGHT_PROT 0

#define LWIP_IPV4 1
#define LWIP_IPV6 0
#define LWIP_NETCONN 0
#define LWIP_SOCKET 0
#define LWIP_UDP 1
#define LWIP_TCP 1
#define LWIP_RAW 1

#define ARP_QUEUEING 1
#define ARP_QUEUE_LEN 16

#define MEMP_NUM_RAW_PCB 32
#define MEMP_NUM_UDP_PCB 32
#define MEMP_NUM_TCP_PCB 32
#define MEMP_NUM_TCP_PCB_LISTEN 32

#define TCP_MSS 1024
#define TCP_SND_BUF (8 * TCP_MSS)
#define TCP_WND (8 * TCP_MSS)
#define MEMP_NUM_TCP_SEG 32

#define CHECKSUM_CHECK_TCP 0
#define CHECKSUM_CHECK_IP 0
#define CHECKSUM_CHECK_UDP 0

#define CHECKSUM_GEN_TCP 0
#define CHECKSUM_GEN_IP 0
#define CHECKSUM_GEN_UDP 0

#define TCP_LISTEN_BACKLOG 1
#define LWIP_NETIF_STATUS_CALLBACK 1

// HACK: DirectPipe needs a TCU_PKG_SIZE aligned read buffer...
#define MEM_ALIGNMENT 8u

//#define LWIP_DEBUG
#ifdef LWIP_DEBUG

#define ETHARP_DEBUG     LWIP_DBG_ON
#define NETIF_DEBUG      LWIP_DBG_ON
#define PBUF_DEBUG       LWIP_DBG_ON
#define API_LIB_DEBUG    LWIP_DBG_ON
#define API_MSG_DEBUG    LWIP_DBG_ON
#define SOCKETS_DEBUG    LWIP_DBG_ON
#define ICMP_DEBUG       LWIP_DBG_ON
#define IGMP_DEBUG       LWIP_DBG_ON
#define INET_DEBUG       LWIP_DBG_ON
#define IP_DEBUG         LWIP_DBG_ON
#define IP_REASS_DEBUG   LWIP_DBG_ON
#define RAW_DEBUG        LWIP_DBG_ON
#define MEM_DEBUG        LWIP_DBG_ON
#define MEMP_DEBUG       LWIP_DBG_ON
#define SYS_DEBUG        LWIP_DBG_ON
#define TIMERS_DEBUG     LWIP_DBG_ON
#define TCP_DEBUG        LWIP_DBG_ON
#define TCP_INPUT_DEBUG  LWIP_DBG_ON
#define TCP_FR_DEBUG     LWIP_DBG_ON
#define TCP_RTO_DEBUG    LWIP_DBG_ON
#define TCP_CWND_DEBUG   LWIP_DBG_ON
#define TCP_WND_DEBUG    LWIP_DBG_ON
#define TCP_OUTPUT_DEBUG LWIP_DBG_ON
#define TCP_RST_DEBUG    LWIP_DBG_ON
#define TCP_QLEN_DEBUG   LWIP_DBG_ON
#define UDP_DEBUG        LWIP_DBG_ON
#define TCPIP_DEBUG      LWIP_DBG_ON
#define SLIP_DEBUG       LWIP_DBG_ON
#define DHCP_DEBUG       LWIP_DBG_ON
#define AUTOIP_DEBUG     LWIP_DBG_ON
#define DNS_DEBUG        LWIP_DBG_ON
#define IP6_DEBUG        LWIP_DBG_ON

#define LWIP_DBG_TYPES_ON (LWIP_DBG_ON|LWIP_DBG_TRACE|LWIP_DBG_STATE|LWIP_DBG_FRESH|LWIP_DBG_HALT)

#endif


#endif /* LWIP_LWIPOPTS_H */
