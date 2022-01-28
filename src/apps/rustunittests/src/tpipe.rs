/*
 * Copyright (C) 2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

use m3::col::String;
use m3::com::MemGate;
use m3::errors::Code;
use m3::io::{self, Read};
use m3::kif;
use m3::pes::{Activity, VPEArgs, PE, VPE};
use m3::session::Pipes;
use m3::test;
use m3::vfs::{BufReader, IndirectPipe};
use m3::{println, wv_assert_eq, wv_assert_ok, wv_run_test};

pub fn run(t: &mut dyn test::WvTester) {
    wv_run_test!(t, child_to_parent);
    wv_run_test!(t, parent_to_child);
    wv_run_test!(t, child_to_child);
    wv_run_test!(t, exec_child_to_child);
    wv_run_test!(t, writer_quit);
    wv_run_test!(t, reader_quit);
}

fn child_to_parent() {
    let pipeserv = wv_assert_ok!(Pipes::new("pipes"));
    let pipe_mem = wv_assert_ok!(MemGate::new(0x10000, kif::Perm::RW));
    let pipe = wv_assert_ok!(IndirectPipe::new(&pipeserv, &pipe_mem, 0x10000));

    let pe = wv_assert_ok!(PE::get("clone|own"));
    let mut vpe = wv_assert_ok!(VPE::new_with(pe, VPEArgs::new("writer")));
    vpe.files().set(
        io::STDOUT_FILENO,
        VPE::cur().files().get(pipe.writer_fd()).unwrap(),
    );
    wv_assert_ok!(vpe.obtain_fds());

    let act = wv_assert_ok!(vpe.run(|| {
        println!("This is a test!");
        0
    }));

    pipe.close_writer();

    let input = VPE::cur().files().get(pipe.reader_fd()).unwrap();
    let s = wv_assert_ok!(input.borrow_mut().read_to_string());
    wv_assert_eq!(s, "This is a test!\n");

    wv_assert_eq!(act.wait(), Ok(0));
}

fn parent_to_child() {
    let pipeserv = wv_assert_ok!(Pipes::new("pipes"));
    let pipe_mem = wv_assert_ok!(MemGate::new(0x10000, kif::Perm::RW));
    let pipe = wv_assert_ok!(IndirectPipe::new(&pipeserv, &pipe_mem, 0x10000));

    let pe = wv_assert_ok!(PE::get("clone|own"));
    let mut vpe = wv_assert_ok!(VPE::new_with(pe, VPEArgs::new("reader")));
    vpe.files().set(
        io::STDIN_FILENO,
        VPE::cur().files().get(pipe.reader_fd()).unwrap(),
    );
    wv_assert_ok!(vpe.obtain_fds());

    let act = wv_assert_ok!(vpe.run(|| {
        let s = wv_assert_ok!(io::stdin().read_to_string());
        wv_assert_eq!(s, "This is a test!\n");
        0
    }));

    pipe.close_reader();

    let output = VPE::cur().files().get(pipe.writer_fd()).unwrap();
    wv_assert_eq!(output.borrow_mut().write(b"This is a test!\n"), Ok(16));

    pipe.close_writer();

    wv_assert_eq!(act.wait(), Ok(0));
}

fn child_to_child() {
    let pipeserv = wv_assert_ok!(Pipes::new("pipes"));
    let pipe_mem = wv_assert_ok!(MemGate::new(0x10000, kif::Perm::RW));
    let pipe = wv_assert_ok!(IndirectPipe::new(&pipeserv, &pipe_mem, 0x10000));

    let pe1 = wv_assert_ok!(PE::get("clone|own"));
    let pe2 = wv_assert_ok!(PE::get("clone|own"));
    let mut writer = wv_assert_ok!(VPE::new_with(pe1, VPEArgs::new("writer")));
    let mut reader = wv_assert_ok!(VPE::new_with(pe2, VPEArgs::new("reader")));
    writer.files().set(
        io::STDOUT_FILENO,
        VPE::cur().files().get(pipe.writer_fd()).unwrap(),
    );
    reader.files().set(
        io::STDIN_FILENO,
        VPE::cur().files().get(pipe.reader_fd()).unwrap(),
    );
    wv_assert_ok!(writer.obtain_fds());
    wv_assert_ok!(reader.obtain_fds());

    let wr_act = wv_assert_ok!(writer.run(|| {
        println!("This is a test!");
        0
    }));

    let rd_act = wv_assert_ok!(reader.run(|| {
        let s = wv_assert_ok!(io::stdin().read_to_string());
        wv_assert_eq!(s, "This is a test!\n");
        0
    }));

    pipe.close_reader();
    pipe.close_writer();

    wv_assert_eq!(wr_act.wait(), Ok(0));
    wv_assert_eq!(rd_act.wait(), Ok(0));
}

fn exec_child_to_child() {
    let pipeserv = wv_assert_ok!(Pipes::new("pipes"));
    let pipe_mem = wv_assert_ok!(MemGate::new(0x10000, kif::Perm::RW));
    let pipe = wv_assert_ok!(IndirectPipe::new(&pipeserv, &pipe_mem, 0x10000));

    let pe1 = wv_assert_ok!(PE::get("clone|own"));
    let pe2 = wv_assert_ok!(PE::get("clone|own"));
    let mut writer = wv_assert_ok!(VPE::new_with(pe1, VPEArgs::new("writer")));
    let mut reader = wv_assert_ok!(VPE::new_with(pe2, VPEArgs::new("reader")));
    writer.files().set(
        io::STDOUT_FILENO,
        VPE::cur().files().get(pipe.writer_fd()).unwrap(),
    );
    reader.files().set(
        io::STDIN_FILENO,
        VPE::cur().files().get(pipe.reader_fd()).unwrap(),
    );
    wv_assert_ok!(writer.obtain_fds());
    wv_assert_ok!(reader.obtain_fds());

    let wr_act = wv_assert_ok!(writer.exec(&["/bin/hello"]));

    let rd_act = wv_assert_ok!(reader.run(|| {
        let s = wv_assert_ok!(io::stdin().read_to_string());
        wv_assert_eq!(s, "Hello World\n");
        0
    }));

    pipe.close_reader();
    pipe.close_writer();

    wv_assert_eq!(wr_act.wait(), Ok(0));
    wv_assert_eq!(rd_act.wait(), Ok(0));
}

fn writer_quit() {
    let pipeserv = wv_assert_ok!(Pipes::new("pipes"));
    let pipe_mem = wv_assert_ok!(MemGate::new(0x10000, kif::Perm::RW));
    let pipe = wv_assert_ok!(IndirectPipe::new(&pipeserv, &pipe_mem, 0x10000));

    let pe = wv_assert_ok!(PE::get("clone|own"));
    let mut vpe = wv_assert_ok!(VPE::new_with(pe, VPEArgs::new("writer")));
    vpe.files().set(
        io::STDOUT_FILENO,
        VPE::cur().files().get(pipe.writer_fd()).unwrap(),
    );
    wv_assert_ok!(vpe.obtain_fds());

    let act = wv_assert_ok!(vpe.run(|| {
        println!("This is a test!");
        println!("This is a test!");
        0
    }));

    pipe.close_writer();

    {
        let input = VPE::cur().files().get_ref(pipe.reader_fd()).unwrap();
        let mut reader = BufReader::new(input);
        let mut s = String::new();
        wv_assert_eq!(reader.read_line(&mut s), Ok(15));
        wv_assert_eq!(s, "This is a test!");
        s.clear();
        wv_assert_eq!(reader.read_line(&mut s), Ok(15));
        wv_assert_eq!(s, "This is a test!");
        s.clear();
        wv_assert_eq!(reader.read_line(&mut s), Ok(0));
        wv_assert_eq!(s, "");
    }

    wv_assert_eq!(act.wait(), Ok(0));
}

fn reader_quit() {
    let pipeserv = wv_assert_ok!(Pipes::new("pipes"));
    let pipe_mem = wv_assert_ok!(MemGate::new(0x10000, kif::Perm::RW));
    let pipe = wv_assert_ok!(IndirectPipe::new(&pipeserv, &pipe_mem, 0x10000));

    let pe = wv_assert_ok!(PE::get("clone|own"));
    let mut vpe = wv_assert_ok!(VPE::new_with(pe, VPEArgs::new("reader")));
    vpe.files().set(
        io::STDIN_FILENO,
        VPE::cur().files().get(pipe.reader_fd()).unwrap(),
    );
    wv_assert_ok!(vpe.obtain_fds());

    let act = wv_assert_ok!(vpe.run(|| {
        let mut s = String::new();
        wv_assert_eq!(io::stdin().read_line(&mut s), Ok(15));
        wv_assert_eq!(s, "This is a test!");
        0
    }));

    pipe.close_reader();

    let output = VPE::cur().files().get(pipe.writer_fd()).unwrap();
    loop {
        let res = output.borrow_mut().write(b"This is a test!\n");
        match res {
            Ok(count) => wv_assert_eq!(count, 16),
            Err(e) => {
                wv_assert_eq!(e.code(), Code::EndOfFile);
                break;
            },
        }
    }

    pipe.close_writer();

    wv_assert_eq!(act.wait(), Ok(0));
}
