/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
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

use m3::col::String;
use m3::com::MemGate;
use m3::errors::Code;
use m3::io::{self, Read, Write};
use m3::kif;
use m3::session::Pipes;
use m3::test::{DefaultWvTester, WvTester};
use m3::tiles::{ActivityArgs, ChildActivity, RunningActivity, Tile};
use m3::vfs::{BufReader, IndirectPipe};
use m3::{println, wv_assert_eq, wv_assert_ok, wv_run_test};

pub fn run(t: &mut dyn WvTester) {
    wv_run_test!(t, child_to_parent);
    wv_run_test!(t, parent_to_child);
    wv_run_test!(t, child_to_child);
    wv_run_test!(t, exec_child_to_child);
    wv_run_test!(t, writer_quit);
    wv_run_test!(t, reader_quit);
}

fn child_to_parent(t: &mut dyn WvTester) {
    let pipeserv = wv_assert_ok!(Pipes::new("pipes"));
    let pipe_mem = wv_assert_ok!(MemGate::new(0x10000, kif::Perm::RW));
    let pipe = wv_assert_ok!(IndirectPipe::new(&pipeserv, &pipe_mem, 0x10000));

    let tile = wv_assert_ok!(Tile::get("compat|own"));
    let mut act = wv_assert_ok!(ChildActivity::new_with(tile, ActivityArgs::new("writer")));
    act.add_file(io::STDOUT_FILENO, pipe.writer().unwrap().fd());

    let act = wv_assert_ok!(act.run(|| {
        println!("This is a test!");
        Ok(())
    }));

    pipe.close_writer();

    let mut input = pipe.reader().unwrap();
    let s = wv_assert_ok!(input.read_to_string());
    wv_assert_eq!(t, s, "This is a test!\n");

    pipe.close_reader();

    wv_assert_eq!(t, act.wait(), Ok(Code::Success));
}

fn parent_to_child(t: &mut dyn WvTester) {
    let pipeserv = wv_assert_ok!(Pipes::new("pipes"));
    let pipe_mem = wv_assert_ok!(MemGate::new(0x10000, kif::Perm::RW));
    let pipe = wv_assert_ok!(IndirectPipe::new(&pipeserv, &pipe_mem, 0x10000));

    let tile = wv_assert_ok!(Tile::get("compat|own"));
    let mut act = wv_assert_ok!(ChildActivity::new_with(tile, ActivityArgs::new("reader")));
    act.add_file(io::STDIN_FILENO, pipe.reader().unwrap().fd());

    let act = wv_assert_ok!(act.run(|| {
        let mut t = DefaultWvTester::default();
        let s = wv_assert_ok!(io::stdin().read_to_string());
        wv_assert_eq!(t, s, "This is a test!\n");
        Ok(())
    }));

    pipe.close_reader();

    let mut output = pipe.writer().unwrap();
    wv_assert_eq!(t, output.write(b"This is a test!\n"), Ok(16));

    pipe.close_writer();

    wv_assert_eq!(t, act.wait(), Ok(Code::Success));
}

fn child_to_child(t: &mut dyn WvTester) {
    let pipeserv = wv_assert_ok!(Pipes::new("pipes"));
    let pipe_mem = wv_assert_ok!(MemGate::new(0x10000, kif::Perm::RW));
    let pipe = wv_assert_ok!(IndirectPipe::new(&pipeserv, &pipe_mem, 0x10000));

    let tile1 = wv_assert_ok!(Tile::get("compat|own"));
    let tile2 = wv_assert_ok!(Tile::get("compat|own"));
    let mut writer = wv_assert_ok!(ChildActivity::new_with(tile1, ActivityArgs::new("writer")));
    let mut reader = wv_assert_ok!(ChildActivity::new_with(tile2, ActivityArgs::new("reader")));
    writer.add_file(io::STDOUT_FILENO, pipe.writer().unwrap().fd());
    reader.add_file(io::STDIN_FILENO, pipe.reader().unwrap().fd());

    let wr_act = wv_assert_ok!(writer.run(|| {
        println!("This is a test!");
        Ok(())
    }));

    let rd_act = wv_assert_ok!(reader.run(|| {
        let mut t = DefaultWvTester::default();
        let s = wv_assert_ok!(io::stdin().read_to_string());
        wv_assert_eq!(t, s, "This is a test!\n");
        Ok(())
    }));

    pipe.close_reader();
    pipe.close_writer();

    wv_assert_eq!(t, wr_act.wait(), Ok(Code::Success));
    wv_assert_eq!(t, rd_act.wait(), Ok(Code::Success));
}

fn exec_child_to_child(t: &mut dyn WvTester) {
    let pipeserv = wv_assert_ok!(Pipes::new("pipes"));
    let pipe_mem = wv_assert_ok!(MemGate::new(0x10000, kif::Perm::RW));
    let pipe = wv_assert_ok!(IndirectPipe::new(&pipeserv, &pipe_mem, 0x10000));

    let tile1 = wv_assert_ok!(Tile::get("compat|own"));
    let tile2 = wv_assert_ok!(Tile::get("compat|own"));
    let mut writer = wv_assert_ok!(ChildActivity::new_with(tile1, ActivityArgs::new("writer")));
    let mut reader = wv_assert_ok!(ChildActivity::new_with(tile2, ActivityArgs::new("reader")));
    writer.add_file(io::STDOUT_FILENO, pipe.writer().unwrap().fd());
    reader.add_file(io::STDIN_FILENO, pipe.reader().unwrap().fd());

    let wr_act = wv_assert_ok!(writer.exec(&["/bin/hello"]));

    let rd_act = wv_assert_ok!(reader.run(|| {
        let mut t = DefaultWvTester::default();
        let s = wv_assert_ok!(io::stdin().read_to_string());
        wv_assert_eq!(t, s, "Hello World\n");
        Ok(())
    }));

    pipe.close_reader();
    pipe.close_writer();

    wv_assert_eq!(t, wr_act.wait(), Ok(Code::Success));
    wv_assert_eq!(t, rd_act.wait(), Ok(Code::Success));
}

fn writer_quit(t: &mut dyn WvTester) {
    let pipeserv = wv_assert_ok!(Pipes::new("pipes"));
    let pipe_mem = wv_assert_ok!(MemGate::new(0x10000, kif::Perm::RW));
    let pipe = wv_assert_ok!(IndirectPipe::new(&pipeserv, &pipe_mem, 0x10000));

    let tile = wv_assert_ok!(Tile::get("compat|own"));
    let mut act = wv_assert_ok!(ChildActivity::new_with(tile, ActivityArgs::new("writer")));
    act.add_file(io::STDOUT_FILENO, pipe.writer().unwrap().fd());

    let act = wv_assert_ok!(act.run(|| {
        println!("This is a test!");
        println!("This is a test!");
        Ok(())
    }));

    pipe.close_writer();

    let input = pipe.reader().unwrap();
    let mut reader = BufReader::new(input);
    let mut s = String::new();
    wv_assert_eq!(t, reader.read_line(&mut s), Ok(15));
    wv_assert_eq!(t, s, "This is a test!");
    s.clear();
    wv_assert_eq!(t, reader.read_line(&mut s), Ok(15));
    wv_assert_eq!(t, s, "This is a test!");
    s.clear();
    wv_assert_eq!(t, reader.read_line(&mut s), Ok(0));
    wv_assert_eq!(t, s, "");

    pipe.close_reader();

    wv_assert_eq!(t, act.wait(), Ok(Code::Success));
}

fn reader_quit(t: &mut dyn WvTester) {
    let pipeserv = wv_assert_ok!(Pipes::new("pipes"));
    let pipe_mem = wv_assert_ok!(MemGate::new(0x10000, kif::Perm::RW));
    let pipe = wv_assert_ok!(IndirectPipe::new(&pipeserv, &pipe_mem, 0x10000));

    let tile = wv_assert_ok!(Tile::get("compat|own"));
    let mut act = wv_assert_ok!(ChildActivity::new_with(tile, ActivityArgs::new("reader")));
    act.add_file(io::STDIN_FILENO, pipe.reader().unwrap().fd());

    let act = wv_assert_ok!(act.run(|| {
        let mut t = DefaultWvTester::default();
        let mut s = String::new();
        wv_assert_eq!(t, io::stdin().read_line(&mut s), Ok(15));
        wv_assert_eq!(t, s, "This is a test!");
        Ok(())
    }));

    pipe.close_reader();

    let mut output = pipe.writer().unwrap();
    loop {
        let res = output.write(b"This is a test!\n");
        match res {
            Ok(count) => wv_assert_eq!(t, count, 16),
            Err(e) => {
                wv_assert_eq!(t, e.code(), Code::EndOfFile);
                break;
            },
        }
    }

    pipe.close_writer();

    wv_assert_eq!(t, act.wait(), Ok(Code::Success));
}
