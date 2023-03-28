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

use lazy_static::lazy_static;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use regex::Regex;
use smoltcp::wire;
use std::convert::TryFrom;
use std::io::Write;
use std::io::{self, BufRead};

use crate::error::Error;

#[derive(IntoPrimitive, TryFromPrimitive)]
#[repr(u64)]
pub enum NetLogEvent {
    SubmitData = 1,
    SentPacket,
    RecvPacket,
    FetchData,
    RecvConnected,
    RecvClosed,
    RecvRemoteClosed,
    StartedWaiting,
    StoppedWaiting,
}

#[derive(Default)]
struct Stats {
    submitted: usize,
    sent: usize,
    received: usize,
    fetched: usize,
}

#[derive(Default)]
struct State {
    last_received: bool,
    last_bytes: usize,
    pkt_buf: Vec<u8>,
}

pub struct NIC {
    idx: usize,
    name: String,
    stats: Stats,
}

impl NIC {
    pub fn new(idx: usize, name: String) -> Self {
        Self {
            idx,
            name,
            stats: Stats::default(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

pub struct Net {
    tile: u64,
    name: String,
    full_name: String,
    stats: Stats,
}

impl Net {
    pub fn new(tile: u64, name: String, nic: &str) -> Self {
        Self {
            tile,
            full_name: format!("{}->{}", name, nic),
            name,
            stats: Stats::default(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn full_name(&self) -> &str {
        &self.full_name
    }
}

pub struct App {
    tile: u64,
    full_name: String,
    stats: Stats,
}

impl App {
    pub fn new(tile: u64, name: String, net: &str) -> Self {
        Self {
            tile,
            full_name: format!("{}->{}", name, net),
            stats: Stats::default(),
        }
    }
}

fn translate_nic(
    nics: &mut [NIC],
    state: &mut State,
    writer: &mut io::StdoutLock<'_>,
    line: &str,
) -> Option<bool> {
    if !line.contains("etherlink") {
        return None;
    }

    lazy_static! {
        static ref NIC_REGEX: Regex =
            Regex::new(r"(\d+): C\d+T\d+.etherlink.link(\d+): packet (received|sent): len=(\d+)")
                .unwrap();
        static ref PKT_REGEX: Regex = Regex::new(
            r"(\d+): C\d+T\d+.etherlink.link(\d+): [0-9a-f]+  (([0-9a-f]{2} ?)+ ?([0-9a-f]{2} ?)*).*"
        )
        .unwrap();
    }

    fn get_nic(nics: &mut [NIC], is_recv: bool, link: usize) -> Option<&mut NIC> {
        // the etherlink lines always show the second NIC tile, but different link numbers depending
        // on the NIC that send/received the packet. If the first NIC sends, it uses link0 and the
        // second NIC receives it via link0. If the second NIC sends, it uses link1 and the first
        // NIC receives it via link1.
        let link = if is_recv { 1 - link } else { link };
        nics.iter_mut().find(|n| n.idx == link)
    }

    let line = line.trim();
    if let Some(c) = NIC_REGEX.captures(line) {
        let time = c.get(1)?.as_str();
        let link = c.get(2)?.as_str().parse::<usize>().ok()?;
        let event = c.get(3)?.as_str();
        let len = c.get(4)?.as_str().parse::<usize>().ok()?;

        state.last_received = event == "received";
        state.last_bytes = len;

        let nic = get_nic(nics, state.last_received, link)?;

        let total = if state.last_received {
            nic.stats.received += len;
            nic.stats.received
        }
        else {
            nic.stats.sent += len;
            nic.stats.sent
        };

        writeln!(
            writer,
            "{}: \x1b[1mNET\x1b[0m: {} {} packet of {}b ({}b total)",
            time, nic.name, event, len, total
        )
        .unwrap();
        Some(true)
    }
    else if let Some(c) = PKT_REGEX.captures(line) {
        let time = c.get(1)?.as_str();
        let link = c.get(2)?.as_str().parse::<usize>().ok()?;
        let bytes = c.get(3)?.as_str();

        let nic = get_nic(nics, state.last_received, link)?;

        for b in bytes.split_whitespace() {
            state.pkt_buf.push(u8::from_str_radix(b, 16).ok()?);
        }

        if state.pkt_buf.len() == state.last_bytes {
            let prefix = format!("{}: \x1b[1mNET\x1b[0m: ", time);
            let dump = format!(
                "{}",
                wire::PrettyPrinter::<wire::EthernetFrame<&'static [u8]>>::new("", &state.pkt_buf)
            );

            let mut lines = String::new();
            for line in dump.split('\n') {
                if !lines.is_empty() {
                    lines.push('\n');
                }
                lines.push_str(&prefix);
                lines.push_str(line);
            }

            writeln!(writer, "{}{} packet dump:\n{}", prefix, nic.name, lines).unwrap();

            state.last_bytes = 0;
            state.pkt_buf.clear();
        }

        Some(true)
    }
    else {
        None
    }
}

fn translate_app(
    nets: &mut [Net],
    apps: &mut [App],
    writer: &mut io::StdoutLock<'_>,
    line: &str,
) -> Option<bool> {
    if !line.contains("DEBUG") {
        return None;
    }

    lazy_static! {
        static ref APP_REGEX: Regex =
            Regex::new(r"(\d+): C\d+T(\d+).cpu: DEBUG 0x([a-f0-9]+)").unwrap();
    }

    if let Some(c) = APP_REGEX.captures(line.trim()) {
        let time = c.get(1)?.as_str();
        let tile = c.get(2)?.as_str().parse::<u64>().ok()?;
        let event = u64::from_str_radix(c.get(3)?.as_str(), 16).ok()?;

        let (stats, app_name) = if let Some(a) = apps.iter_mut().find(|a| a.tile == tile) {
            (&mut a.stats, &a.full_name)
        }
        else {
            let net = nets.iter_mut().find(|n| n.tile == tile)?;
            (&mut net.stats, &net.full_name)
        };

        let event_type = NetLogEvent::try_from(event & 0xFF);
        let arg = (event >> 16) as usize;
        let sd = (event >> 8) & 0xFF;

        match event_type {
            Ok(NetLogEvent::SubmitData) => stats.submitted += arg,
            Ok(NetLogEvent::SentPacket) => stats.sent += arg,
            Ok(NetLogEvent::RecvPacket) => stats.received += arg,
            Ok(NetLogEvent::FetchData) => stats.fetched += arg,
            _ => {},
        }

        let event_str = match event_type {
            Ok(NetLogEvent::SubmitData) => format!(
                "[{}] submit data of {}b ({}b total)",
                sd, arg, stats.submitted
            ),
            Ok(NetLogEvent::SentPacket) => {
                format!("[{}] sent packet of {}b ({}b total)", sd, arg, stats.sent)
            },
            Ok(NetLogEvent::RecvPacket) => format!(
                "[{}] recv packet of {}b ({}b total)",
                sd, arg, stats.received
            ),
            Ok(NetLogEvent::FetchData) => {
                format!("[{}] fetch data of {}b ({}b total)", sd, arg, stats.fetched)
            },
            Ok(NetLogEvent::RecvConnected) => format!("[{}] recv connected to port {}", sd, arg),
            Ok(NetLogEvent::RecvClosed) => format!("[{}] recv closed", sd),
            Ok(NetLogEvent::RecvRemoteClosed) => format!("[{}] recv remote closed", sd),
            Ok(NetLogEvent::StartedWaiting) => format!("[{}] started waiting", sd),
            Ok(NetLogEvent::StoppedWaiting) => format!("[{}] stopped waiting", sd),
            _ => format!("unknown event {:#x}", event & 0xFF),
        };

        writeln!(
            writer,
            "{}: \x1b[1mNET\x1b[0m: {} {}",
            time, app_name, event_str
        )
        .unwrap();
        Some(true)
    }
    else {
        None
    }
}

pub fn translate(nics: &mut [NIC], nets: &mut [Net], apps: &mut [App]) -> Result<(), Error> {
    let stdin = io::stdin();
    let mut reader = io::BufReader::new(stdin.lock());

    let stdout = io::stdout();
    let mut writer = stdout.lock();

    let mut state = State::default();
    let mut line = String::new();
    while reader.read_line(&mut line)? != 0 {
        // try to replace the address with the binary and symbol
        if translate_nic(nics, &mut state, &mut writer, &line).is_none()
            && translate_app(nets, apps, &mut writer, &line).is_none()
        {
            // if that failed, just write out the line
            writer.write_all(line.as_bytes())?;
        }
        line.clear();
    }
    Ok(())
}
