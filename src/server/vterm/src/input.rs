/*
 * Copyright (C) 2023 Nils Asmussen, Barkhausen Institut
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

use m3::cell::{RefMut, StaticCell, StaticRefCell};
use m3::col::Vec;
use m3::errors::Code;
use m3::io::{LogFlags, Serial, Write};
use m3::log;
use m3::server::ClientManager;
use m3::tcu::Message;
use m3::vec;
use m3::vfs::{FileEvent, TMode};

use crate::{SessionData, VTermSession};

static BUFFER: StaticRefCell<Vec<u8>> = StaticRefCell::new(Vec::new());
static INPUT: StaticRefCell<Vec<u8>> = StaticRefCell::new(Vec::new());
static EOF: StaticCell<bool> = StaticCell::new(false);
static MODE: StaticCell<TMode> = StaticCell::new(TMode::Cooked);

macro_rules! reply_vmsg_late {
    ( $rgate:expr, $msg:expr, $( $args:expr ),* ) => ({
        let mut msg = m3::mem::MsgBuf::borrow_def();
        m3::build_vmsg!(&mut msg, $( $args ),*);
        $rgate.reply(&msg, $msg)
    });
}

pub fn eof() -> bool {
    EOF.get()
}

pub fn set_eof(eof: bool) {
    EOF.set(eof);
}

pub fn mode() -> TMode {
    MODE.get()
}

pub fn set_mode(mode: TMode) {
    MODE.set(mode);
    INPUT.borrow_mut().clear();
}

pub fn get() -> RefMut<'static, Vec<u8>> {
    INPUT.borrow_mut()
}

pub fn receive_acks(cli: &mut ClientManager<VTermSession>) {
    cli.for_each(|s| match &mut s.data {
        SessionData::Chan(c) => {
            if let Some(rg) = c.notify_rgate() {
                if let Ok(msg) = rg.fetch() {
                    rg.ack_msg(msg).unwrap();
                    // try again to send events, if there are some
                    c.send_events();
                }
            }
        },
        SessionData::Meta => {},
    });
}

pub fn handle_input(cli: &mut ClientManager<VTermSession>, msg: &'static Message) {
    let mut input = INPUT.borrow_mut();
    let mut buffer = BUFFER.borrow_mut();

    log!(
        LogFlags::VTInOut,
        "Got input message with {} bytes",
        msg.header.length()
    );

    let bytes = unsafe { core::slice::from_raw_parts(msg.data.as_ptr(), msg.header.length()) };
    let mut flush = false;
    let mut eof = false;
    if MODE.get() == TMode::Raw {
        input.extend_from_slice(bytes);
    }
    else {
        let mut output = vec![];
        for b in bytes {
            match b {
                // ^D
                0x04 => eof = true,
                // ^C
                0x03 => add_signal(cli),
                // backspace
                0x7f => {
                    output.push(0x08);
                    output.push(b' ');
                    output.push(0x08);
                    buffer.pop();
                },
                b => {
                    if *b == 27 {
                        buffer.push(b'^');
                        output.push(b'^');
                    }
                    else if *b == b'\n' {
                        flush = true;
                    }
                    if *b == b'\n' || !b.is_ascii_control() {
                        buffer.push(*b);
                    }
                },
            }

            if *b == b'\n' || !b.is_ascii_control() {
                output.push(*b);
            }
        }

        if eof || flush {
            input.extend_from_slice(&buffer);
            buffer.clear();
        }
        Serial::new().write(&output).unwrap();
    }

    add_input(cli, eof, eof || flush, &mut input);
}

fn add_signal(cli: &mut ClientManager<VTermSession>) {
    cli.for_each(|s| match &mut s.data {
        SessionData::Chan(c) => {
            c.add_event(FileEvent::SIGNAL);
        },
        SessionData::Meta => {},
    });
}

fn add_input(
    cli: &mut ClientManager<VTermSession>,
    eof: bool,
    mut flush: bool,
    input: &mut RefMut<'_, Vec<u8>>,
) {
    // pass to first session that wants input
    EOF.set(eof);

    let mut input_recv: Option<(&Message, usize, usize)> = None;

    cli.for_each(|s| {
        if flush || !input.is_empty() {
            if let SessionData::Chan(c) = &mut s.data {
                if let Some((msg, pos, len)) = c.fetch_input(input).unwrap() {
                    input_recv = Some((msg, pos, len));
                    flush = false;
                }
                else if c.add_event(FileEvent::INPUT) {
                    flush = false;
                }
            }
        }
    });

    if let Some((msg, pos, len)) = input_recv {
        reply_vmsg_late!(cli.recv_gate(), msg, Code::Success, pos, len - pos).unwrap();
    }
}
