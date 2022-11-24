/*
 * Copyright (C) 2021, Stephan Gerhold <stephan.gerhold@mailbox.tu-dresden.de>
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

#![no_std]

use m3::crypto::HashAlgorithm;
use m3::errors::{Code, Error};
use m3::io::{STDIN_FILENO, STDOUT_FILENO};
use m3::session::{HashInput, HashOutput, HashSession};
use m3::tiles::Activity;
use m3::vfs::{Fd, File, FileRef, OpenFlags, VFS};
use m3::{env, print, println, vec};

fn open_file(path: &str, flags: OpenFlags, stdfd: Fd) -> Result<FileRef<dyn File>, Error> {
    if path != "-" {
        VFS::open(path, flags).map(|f| f.into_generic())
    }
    else {
        Activity::own()
            .files()
            .get(stdfd)
            .ok_or_else(|| Error::new(Code::NoSuchFile))
    }
}

fn hash(
    sess: &mut HashSession,
    path: &str,
    output_bytes: usize,
    output_file: Option<&mut FileRef<dyn File>>,
) -> Result<(), Error> {
    let mut file = open_file(path, OpenFlags::R, STDIN_FILENO)?;
    sess.reset(sess.algo())?;
    file.hash_input(sess, usize::MAX)?;

    if let Some(output_file) = output_file {
        output_file.hash_output(sess, output_bytes).map(|_| ())
    }
    else {
        let mut result = vec![0; output_bytes];
        sess.finish(&mut result)?;
        println!("{}  {}", hex::encode(&result), path);
        Ok(())
    }
}

#[no_mangle]
pub fn main() -> Result<(), Error> {
    let mut args = env::args();
    let program = args.next().unwrap_or("hashsum");
    let algo = match args.next().and_then(HashAlgorithm::from_name) {
        Some(algo) => algo,
        None => {
            print!("Usage: {} <", program);
            let mut sep = "";
            for algo in HashAlgorithm::ALL.iter() {
                print!("{}{}", sep, algo.name);
                sep = "|";
            }
            println!("> [-O <output-bytes>|-o <output-file>] [files...]");
            return Err(Error::new(Code::InvArgs));
        },
    };

    let mut output_file = None;
    let mut output_bytes = algo.output_bytes;
    let mut next;

    loop {
        next = args.next();
        match next {
            Some("-O") => {
                output_bytes = args
                    .next()
                    .expect("Missing argument")
                    .parse()
                    .expect("Failed to parse output size")
            },
            Some("-o") => {
                output_file = Some(
                    open_file(
                        args.next().expect("Missing argument"),
                        OpenFlags::W | OpenFlags::CREATE,
                        STDOUT_FILENO,
                    )
                    .expect("Failed to open output file"),
                );
            },
            _ => break,
        }
    }

    if output_bytes > algo.output_bytes {
        println!(
            "Output size {} larger than hash output size {}",
            output_bytes, algo.output_bytes
        );
        return Err(Error::new(Code::InvArgs));
    }

    let mut sess = HashSession::new("hash", algo).expect("Failed to get hash session");

    let mut res = Ok(());
    next = next.or(Some("-"));
    while let Some(path) = next {
        if let Err(e) = hash(&mut sess, path, output_bytes, output_file.as_mut()) {
            if output_file.as_ref().map(|f| f.fd()) != Some(STDOUT_FILENO) {
                // Avoid printing to standard output if it is used as output file
                println!("{}: {}: {}", program, path, e);
            }
            else {
                panic!("Failed to hash file: {:?}", e)
            }
            res = Err(e);
        }
        next = args.next()
    }

    res
}
