/*
 * Copyright (C) 2022 Nils Asmussen, Barkhausen Institut
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

use core::mem;
use core::str::FromStr;

use base::col::ToString;
use base::errors::{Code, Error, VerboseError};
use base::rc::Rc;
use base::time::TimeDuration;
use base::util::random::LinearCongruentialGenerator;
use base::vec;

use crate::client::Network;
use crate::net::{DGramSocket, DgramSocketArgs, Endpoint, IpAddr, Port, Socket, UdpSocket};
use crate::vfs::{File, FileEvent, FileWaiter};

// based on http://tools.ietf.org/html/rfc1035

const DNS_RECURSION_DESIRED: u16 = 0x100;
const DNS_PORT: Port = 53;

const TYPE_A: u16 = 1; // a host address
const CLASS_IN: u16 = 1; // the internet

#[repr(C, packed)]
struct DNSHeader {
    id: u16,
    flags: u16,
    qd_count: u16,
    an_count: u16,
    ns_count: u16,
    ar_count: u16,
}

#[repr(C, packed)]
struct DNSQuestionEnd {
    ty: u16,
    cls: u16,
}

#[repr(C, packed)]
struct DNSAnswer {
    name: u16,
    ty: u16,
    cls: u16,
    ttl: u32,
    length: u16,
    // this is the data part of the answer, where we currently only support IPv4 addresses
    ip_addr: u32,
}

/// Domain name service resolver
///
/// The DNS type uses [`Network`] to resolve host names to IP addresses.
#[derive(Default)]
pub struct DNS {
    nameserver: IpAddr,
    random: LinearCongruentialGenerator,
}

impl DNS {
    /// Translates the given name into an IP address. If the name is already an IP address, it will
    /// simply be converted into an [`IpAddr`] object. Otherwise, the name will be solved via DNS.
    ///
    /// The timeout specifies the maximum time to wait for the DNS response.
    pub fn get_addr(
        &mut self,
        netmng: Rc<Network>,
        name: &str,
        timeout: TimeDuration,
    ) -> Result<IpAddr, VerboseError> {
        if let Ok(addr) = IpAddr::from_str(name) {
            return Ok(addr);
        }

        self.resolve(netmng, name, timeout)
    }

    /// Resolves the given hostname to an IP address. Note that this method assumes that the name is
    /// not an IP address, but an actual hostname and will therefore always use DNS to resolve the
    /// name. Use [`get_addr`](Self::get_addr) if you don't know whether it's a hostname or an IP
    /// address.
    ///
    /// The timeout specifies the maximum time to wait for the DNS response.
    pub fn resolve(
        &mut self,
        netmng: Rc<Network>,
        name: &str,
        timeout: TimeDuration,
    ) -> Result<IpAddr, VerboseError> {
        if self.nameserver == IpAddr::unspecified() {
            self.nameserver = netmng.nameserver()?;
        }

        let total = mem::size_of::<DNSHeader>() + name.len() + 2 + mem::size_of::<DNSQuestionEnd>();
        // reserve some space for the response as well
        let mut buf = vec![0u8; total.max(1024)];

        let mut sock = UdpSocket::new(DgramSocketArgs::new(netmng))?;

        let txid = self.random.get() as u16;
        Self::generate_request(&mut buf, txid, name)?;
        sock.send_to(&buf[0..total], Endpoint::new(self.nameserver, DNS_PORT))?;

        // wait for the response
        sock.set_blocking(false)?;
        let mut waiter = FileWaiter::default();
        waiter.add(sock.fd(), FileEvent::INPUT);
        waiter.wait_for(timeout);

        let len = sock.recv(&mut buf)?;
        Self::handle_response(&buf[0..len], txid)
    }

    fn generate_request(buf: &mut [u8], txid: u16, name: &str) -> Result<(), VerboseError> {
        // safety: we are still within the allocated vector and DNSHeader has no alignment
        // requirements
        let header = unsafe { &mut *(buf.as_mut_ptr() as *mut DNSHeader) };

        // build DNS request message
        header.id = txid.to_be();
        header.flags = DNS_RECURSION_DESIRED.to_be();
        header.qd_count = 1u16.to_be();
        header.an_count = 0;
        header.ns_count = 0;
        header.ar_count = 0;

        // add hostname
        let hostname_bytes = &mut buf[mem::size_of::<DNSHeader>()..];
        Self::convert_hostname(hostname_bytes, name)?;

        // safety: we are still within the allocated vector and DNSQuestionEnd has no alignment
        // requirements
        let qend = unsafe {
            &mut *(buf
                .as_mut_ptr()
                .add(mem::size_of::<DNSHeader>() + name.len() + 2)
                as *mut DNSQuestionEnd)
        };
        qend.ty = TYPE_A.to_be();
        qend.cls = CLASS_IN.to_be();

        Ok(())
    }

    fn convert_hostname(dst: &mut [u8], src: &str) -> Result<(), Error> {
        let mut idx = src.len();
        let mut part_length = 0i8;

        // we start with the \0 at the end
        dst[idx + 1] = b'\0';

        for b in src.bytes().rev() {
            if b == b'.' {
                dst[idx] = part_length as u8;
                part_length = 0;
            }
            else {
                dst[idx] = b;
                part_length = part_length
                    .checked_add(1)
                    .ok_or_else(|| Error::new(Code::InvArgs))?;
            }
            idx -= 1;
        }

        dst[idx] = part_length as u8;
        Ok(())
    }

    fn handle_response(buf: &[u8], txid: u16) -> Result<IpAddr, VerboseError> {
        if buf.len() < mem::size_of::<DNSHeader>() {
            return Err(VerboseError::new(
                Code::NotFound,
                "Invalid DNS response".to_string(),
            ));
        }

        // safety: the length is sufficient now and heap allocations are 16-byte aligned
        let header = unsafe { &*(buf.as_ptr() as *const DNSHeader) };
        if u16::from_be(header.id) != txid {
            return Err(VerboseError::new(
                Code::NotFound,
                "Received DNS response with wrong transaction id".to_string(),
            ));
        }

        let questions = u16::from_be(header.qd_count);
        let answers = u16::from_be(header.an_count);

        let answers_off = Self::skip_questions(buf, questions as usize);
        Self::parse_answers(buf, answers as usize, answers_off)
    }

    fn skip_questions(buf: &[u8], count: usize) -> usize {
        let mut off = mem::size_of::<DNSHeader>();
        for _ in 0..count {
            let qlen = Self::question_length(&buf[off..]);
            off += qlen + mem::size_of::<DNSQuestionEnd>();
        }
        off
    }

    fn question_length(buf: &[u8]) -> usize {
        let mut total = 0;
        let mut off = 0;
        while off < buf.len() && buf[off] != 0 {
            let len = buf[off] as usize;
            // skip this name-part
            total += len + 1;
            off += len + 1;
        }
        // skip zero ending, too
        total + 1
    }

    fn parse_answers(buf: &[u8], start: usize, count: usize) -> Result<IpAddr, VerboseError> {
        for off in start..start + count {
            if off + mem::size_of::<DNSAnswer>() > buf.len() {
                return Err(VerboseError::new(
                    Code::NotFound,
                    "Invalid DNS response".to_string(),
                ));
            }

            // safety: we check above whether we are in bounds and DNSAnswer has no alignment req.
            let ans = unsafe { &*(buf.as_ptr().add(off) as *const DNSAnswer) };
            if u16::from_be(ans.ty) == TYPE_A
                && u16::from_be(ans.length) == mem::size_of::<IpAddr>() as u16
            {
                return Ok(IpAddr::new_from_raw(u32::from_be(ans.ip_addr)));
            }
        }

        Err(VerboseError::new(
            Code::NotFound,
            "No IPv4 address in DNS response".to_string(),
        ))
    }
}
