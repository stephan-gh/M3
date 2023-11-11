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

use hex_literal::hex;

use m3::client::{HashInput, HashOutput, HashSession, Pipes};
use m3::col::Vec;
use m3::com::{MemCap, MemGate, Perm};
use m3::crypto::{HashAlgorithm, HashType};
use m3::errors::{Code, Error};
use m3::io;
use m3::io::{Read, Write};
use m3::mem::VirtAddr;
use m3::test::{DefaultWvTester, WvTester};
use m3::tiles::{Activity, ChildActivity, RunningActivity, RunningProgramActivity, Tile};
use m3::vfs::{File, FileRef, IndirectPipe, OpenFlags, Seek, SeekMode, VFS};
use m3::{format, wv_assert_eq, wv_assert_err, wv_assert_ok, wv_assert_some, wv_run_test};
use m3::{println, tmif, util, vec};

pub fn run(t: &mut dyn WvTester) {
    wv_run_test!(t, hash_empty);
    wv_run_test!(t, hash_mapped_mem);
    wv_run_test!(t, hash_file);
    wv_run_test!(t, seek_then_hash_file);
    wv_run_test!(t, read0_then_hash_file);
    wv_run_test!(t, write0_then_hash_file);
    wv_run_test!(t, read_then_hash_file);
    wv_run_test!(t, shake_and_hash);
    wv_run_test!(t, shake_and_hash_file);
    wv_run_test!(t, shake_and_hash_pipe);
}

fn _hash_empty(
    t: &mut dyn WvTester,
    hash: &mut HashSession,
    algo: &'static HashAlgorithm,
    expected: &[u8],
) {
    wv_assert_ok!(hash.reset(algo));

    let mut buf = vec![0u8; algo.output_bytes];
    wv_assert_ok!(hash.finish(&mut buf));
    wv_assert_err!(t, hash.finish(&mut buf), Code::InvArgs); // Can only request hash once

    wv_assert_eq!(t, &buf, expected);
}

fn hash_empty(t: &mut dyn WvTester) {
    let mut hash = wv_assert_ok!(HashSession::new("hash", &HashAlgorithm::SHA3_256));

    _hash_empty(
        t,
        &mut hash,
        &HashAlgorithm::SHA3_224,
        &hex!("6b4e03423667dbb73b6e15454f0eb1abd4597f9a1b078e3f5b5a6bc7"),
    );
    _hash_empty(
        t,
        &mut hash,
        &HashAlgorithm::SHA3_256,
        &hex!("a7ffc6f8bf1ed76651c14756a061d662f580ff4de43b49fa82d80a4b80f8434a"),
    );
    _hash_empty(
        t,
        &mut hash,
        &HashAlgorithm::SHA3_384,
        &hex!(
            "0c63a75b845e4f7d01107d852e4c2485c51a50aaaa94fc61995e71bbee983a2ac3713831264adb47fb6bd1e058d5f004"
        ),
    );
    _hash_empty(
        t,
        &mut hash,
        &HashAlgorithm::SHA3_512,
        &hex!(
            "a69f73cca23a9ac5c8b567dc185a756e97c982164fe25859e0d1dcc1475c80a615b2123af1f5f94c11e3e9402c3ac558f500199d95b6d3e301758586281dcd26"
        ),
    );
}

fn hash_mapped_mem(t: &mut dyn WvTester) {
    if !Activity::own().tile_desc().has_virtmem() {
        println!("No virtual memory; skipping hash_mapped_mem test");
        return;
    }

    const ADDR: VirtAddr = VirtAddr::new(0x3000_0000);
    const SIZE: usize = 32 * 1024; // 32 KiB
    let mcap = wv_assert_ok!(MemCap::new(SIZE, Perm::RW));

    // Prepare hash session
    let hash = wv_assert_ok!(HashSession::new("hash", &HashAlgorithm::SHA3_256));
    wv_assert_ok!(hash.ep().configure(mcap.sel()));

    // Map memory
    wv_assert_ok!(
        Activity::own()
            .pager()
            .unwrap()
            .map_mem(ADDR, mcap.sel(), SIZE, Perm::RW)
    );

    // Fill memory with some data
    let buf = unsafe { util::slice_for_mut(ADDR.as_mut_ptr(), SIZE) };
    let mut i = 0u8;
    for b in buf {
        *b = i;
        i = i.wrapping_add(1);
    }

    // Flush the cache, otherwise the writes above might not have ended up in
    // physical memory yet. It should be enough to flush the memory for the buffer
    // but the TileMux does not seem to provide that functionality at the moment.
    wv_assert_ok!(tmif::flush_invalidate());

    // Check resulting hash
    let mut buf = [0u8; HashAlgorithm::SHA3_256.output_bytes];
    wv_assert_ok!(hash.input(0, SIZE));
    wv_assert_ok!(hash.finish(&mut buf));
    wv_assert_eq!(
        t,
        &buf,
        &hex!("3d69687d744b35b2c3a757240c5dc0f05a99f2402737cd776b8dfca8b6ecc667")
    );

    // Unmap the memory again. This is important otherwise act.run(...) will fail below
    wv_assert_ok!(Activity::own().pager().unwrap().unmap(ADDR));
}

fn _hash_file(
    t: &mut dyn WvTester,
    file: &mut FileRef<dyn File>,
    hash: &mut HashSession,
    algo: &'static HashAlgorithm,
    expected: &[u8],
) -> Result<(), Error> {
    wv_assert_ok!(hash.reset(algo));
    wv_assert_ok!(file.hash_input(hash, usize::MAX));

    let mut buf = vec![0u8; algo.output_bytes];
    wv_assert_ok!(hash.finish(&mut buf));

    wv_assert_eq!(t, &buf, expected);
    match buf == expected {
        true => Ok(()),
        false => Err(Error::new(Code::Unspecified)),
    }
}

fn _to_hex_bytes(s: &str) -> Vec<u8> {
    let mut res = Vec::with_capacity(s.len());
    let mut i = 0;
    while i + 1 < s.len() {
        let c1 = s.chars().nth(i).unwrap();
        let c2 = s.chars().nth(i + 1).unwrap();
        let num = c1.to_digit(16).unwrap() * 16 + c2.to_digit(16).unwrap();
        res.push(num as u8);
        i += 2;
    }
    res
}

// Hash files asynchronously on separate activities to test context switching.
// The time slice is also chosen quite low so that there are actually context switches happening.

fn _hash_file_start(
    algo: &'static HashAlgorithm,
    file: &FileRef<dyn File>,
    expected: &str,
) -> RunningProgramActivity {
    let tile = wv_assert_ok!(Tile::get("compat|own"));
    let mut act = wv_assert_ok!(ChildActivity::new(tile, algo.name));

    act.add_file(io::STDIN_FILENO, file.fd());

    let mut dst = act.data_sink();
    dst.push(algo.ty);
    dst.push(expected);

    wv_assert_ok!(act.run(|| {
        let mut t = DefaultWvTester::default();
        let mut src = Activity::own().data_source();
        let ty: HashType = src.pop().unwrap();
        let expected_bytes = _to_hex_bytes(src.pop().unwrap());

        let algo = HashAlgorithm::from_type(ty).unwrap();
        let mut hash = wv_assert_ok!(HashSession::new(&format!("hash{}", ty as usize), algo));
        _hash_file(
            &mut t,
            io::stdin().get_mut(),
            &mut hash,
            algo,
            &expected_bytes,
        )
    }))
}

fn hash_file(t: &mut dyn WvTester) {
    let file = wv_assert_ok!(VFS::open(
        "/movies/starwars.txt",
        OpenFlags::R | OpenFlags::NEW_SESS
    ));
    let file = file.into_generic();

    let hashes = [
        (
            &HashAlgorithm::SHA3_512,
            "7cf025af9e77e310ce912d28ae854f37aa62eb1fae81cc9b8a26dac81eb2bd6e9e277e419af033eabf8e1ffb663c06e0d2349b03f4262c4fd0a9e74d9156ca94",
        ),
        (
            &HashAlgorithm::SHA3_384,
            "261b44d87914504a0eb6b4dbe87836856427a7e57d7e3e4a1c559d99937ef6d26f360373df9202dcafc318b6ca6c21c5",
        ),
        (
            &HashAlgorithm::SHA3_256,
            "a1cefebeb163af9c359039b0a75e9c88609c0f670e5d35fdc4be822b64f50f31",
        ),
        (
            &HashAlgorithm::SHA3_224,
            "2969482b56d4a98bc46bb298b264d464d75f6a78265df3b98f6dd017",
        ),
    ];

    for (algo, hash) in &hashes {
        let closure = _hash_file_start(algo, &file, hash);
        wv_assert_eq!(t, closure.wait(), Ok(Code::Success));
    }
}

fn seek_then_hash_file(t: &mut dyn WvTester) {
    let mut hash = wv_assert_ok!(HashSession::new("hash", &HashAlgorithm::SHA3_256));
    let mut file = wv_assert_ok!(VFS::open(
        "/movies/starwars.txt",
        OpenFlags::R | OpenFlags::NEW_SESS
    ));

    wv_assert_ok!(file.seek(1 * 1024 * 1024, SeekMode::Cur));
    _hash_file(
        t,
        &mut file.into_generic(),
        &mut hash,
        &HashAlgorithm::SHA3_256,
        &hex!("56ea8bb7197d843cfe0cb6e80f6b02e6e1a14b026e6628b91f09cb5f60ca4e01"),
    )
    .unwrap();
}

fn read0_then_hash_file(t: &mut dyn WvTester) {
    let mut hash = wv_assert_ok!(HashSession::new("hash", &HashAlgorithm::SHA3_256));
    let mut file = wv_assert_ok!(VFS::open(
        "/testfile.txt",
        OpenFlags::RW | OpenFlags::NEW_SESS
    ));

    // Read zero bytes
    let mut buf = [0u8; 0];
    wv_assert_ok!(file.read(&mut buf));

    _hash_file(
        t,
        &mut file.into_generic(),
        &mut hash,
        &HashAlgorithm::SHA3_256,
        &hex!("0e63e307beb389b2fd7ea292c3bbf2e9e6e1005d82d3620d39c41b22e6db9df8"),
    )
    .unwrap();
}

fn write0_then_hash_file(t: &mut dyn WvTester) {
    let mut hash = wv_assert_ok!(HashSession::new("hash", &HashAlgorithm::SHA3_256));
    let mut file = wv_assert_ok!(VFS::open(
        "/testfile.txt",
        OpenFlags::RW | OpenFlags::NEW_SESS
    ));

    // Write zero bytes
    let buf = [0u8; 0];
    wv_assert_ok!(file.write(&buf));

    _hash_file(
        t,
        &mut file.into_generic(),
        &mut hash,
        &HashAlgorithm::SHA3_256,
        &hex!("0e63e307beb389b2fd7ea292c3bbf2e9e6e1005d82d3620d39c41b22e6db9df8"),
    )
    .unwrap();
}

fn read_then_hash_file(t: &mut dyn WvTester) {
    let mut hash = wv_assert_ok!(HashSession::new("hash", &HashAlgorithm::SHA3_256));
    let mut file = wv_assert_ok!(VFS::open(
        "/testfile.txt",
        OpenFlags::R | OpenFlags::NEW_SESS
    ));

    // Read some bytes
    let res = wv_assert_ok!(file.read_string(4));
    wv_assert_eq!(t, res, "This");

    // Hash rest of the file
    _hash_file(
        t,
        &mut file.into_generic(),
        &mut hash,
        &HashAlgorithm::SHA3_256,
        &hex!("e4a0a34734c9c4bd45fb92cca0204fce0b0188e776632150d5be1083059e934f"),
    )
    .unwrap();
}

const SHAKE_SIZE: usize = 1 * 1024 * 1024; // 1 MiB

fn _shake_and_hash(
    t: &mut dyn WvTester,
    hash: &mut HashSession,
    algo: &'static HashAlgorithm,
    mgate: &MemGate,
    expected_sha3_256: &[u8],
) {
    const SEED: &str = "M3";

    // Generate 1 MiB pseudo-random bytes with seed
    wv_assert_ok!(hash.reset(algo));
    wv_assert_ok!(mgate.write(SEED.as_bytes(), 0)); // Write seed
    wv_assert_ok!(hash.input(0, SEED.len())); // Absorb seed
    wv_assert_ok!(hash.output(0, SHAKE_SIZE));

    // For now, input should not be allowed after output
    wv_assert_err!(t, hash.input(0, SHAKE_SIZE), Code::InvState);

    // Verify generated bytes using hash
    wv_assert_ok!(hash.reset(&HashAlgorithm::SHA3_256));
    wv_assert_ok!(hash.input(0, SHAKE_SIZE));

    let mut buf = [0u8; HashAlgorithm::SHA3_256.output_bytes];
    wv_assert_ok!(hash.finish(&mut buf));
    wv_assert_eq!(t, &buf, expected_sha3_256);
}

fn shake_and_hash(t: &mut dyn WvTester) {
    let mut hash = wv_assert_ok!(HashSession::new("hash", &HashAlgorithm::SHAKE128));
    let mgate = wv_assert_ok!(MemGate::new(SHAKE_SIZE, Perm::RW));
    let mgate_derived = wv_assert_ok!(mgate.derive_cap(0, SHAKE_SIZE, Perm::RW));
    wv_assert_ok!(hash.ep().configure(mgate_derived.sel()));

    _shake_and_hash(
        t,
        &mut hash,
        &HashAlgorithm::SHAKE128,
        &mgate,
        &hex!("8097036d4cafc64911f03e64cbaeee20e07f33d7829ecb60ed5b503b5a1dc341"),
    );
    _shake_and_hash(
        t,
        &mut hash,
        &HashAlgorithm::SHAKE256,
        &mgate,
        &hex!("dcfc0e8e378d10ab8ee0b6f089394eafdf30c790232aff9a0671b701e4b20ba2"),
    );
}

fn _shake_and_hash_file(
    t: &mut dyn WvTester,
    hash: &mut HashSession,
    algo: &'static HashAlgorithm,
    expected_sha3_256: &[u8],
) {
    wv_assert_ok!(hash.reset(algo));

    {
        // Absorb seed
        let mut file = wv_assert_ok!(VFS::open(
            "/movies/starwars.txt",
            OpenFlags::R | OpenFlags::NEW_SESS
        ));
        wv_assert_ok!(file.hash_input(hash, usize::MAX));
    }

    // Squeeze output
    let mut file = wv_assert_ok!(VFS::open(
        "/shake.bin",
        OpenFlags::RW | OpenFlags::CREATE | OpenFlags::NEW_SESS
    ));
    wv_assert_ok!(file.hash_output(hash, SHAKE_SIZE));
    wv_assert_ok!(file.seek(0, SeekMode::Set));

    // Verify generated bytes using hash
    wv_assert_ok!(hash.reset(&HashAlgorithm::SHA3_256));
    wv_assert_ok!(file.hash_input(hash, usize::MAX));

    // Write hash to file
    wv_assert_ok!(file.seek(0, SeekMode::Set));
    wv_assert_ok!(file.hash_output(hash, HashAlgorithm::SHA3_256.output_bytes));

    // Read hash from file
    wv_assert_ok!(file.seek(0, SeekMode::Set));
    let mut buf = [0u8; HashAlgorithm::SHA3_256.output_bytes];
    wv_assert_ok!(file.read_exact(&mut buf));
    wv_assert_eq!(t, &buf, expected_sha3_256);
}

fn shake_and_hash_file(t: &mut dyn WvTester) {
    let mut hash = wv_assert_ok!(HashSession::new("hash", &HashAlgorithm::SHAKE128));

    _shake_and_hash_file(
        t,
        &mut hash,
        &HashAlgorithm::SHAKE128,
        &hex!("95778082448a4548fc7cf32a6793e8d2130f109d497121a7dec0e918d4d724f7"),
    );
    _shake_and_hash_file(
        t,
        &mut hash,
        &HashAlgorithm::SHAKE256,
        &hex!("a67d72c73fd20e36a28a7923fffb73d55c1da05121c08c018673bbfcfb50dcdf"),
    );
}

const PIPE_SHAKE_SIZE: usize = 256 * 1024; // 256 KiB

// echo Pipe! | hashsum shake128 -O 262144 -o - | hashsum sha3-224
fn shake_and_hash_pipe(t: &mut dyn WvTester) {
    let pipes = wv_assert_ok!(Pipes::new("pipes"));

    // Create two pipes
    let imgate = wv_assert_ok!(MemGate::new(0x1000, Perm::RW));
    let ipipe = wv_assert_ok!(IndirectPipe::new(&pipes, imgate));
    let omgate = wv_assert_ok!(MemGate::new(0x10000, Perm::RW));
    let opipe = wv_assert_ok!(IndirectPipe::new(&pipes, omgate));

    // Setup child activity that runs "hashsum shake128 -O 262144 -o -"
    let tile = wv_assert_ok!(Tile::get("compat|own"));
    let mut act = wv_assert_ok!(ChildActivity::new(tile, "shaker"));
    act.add_file(io::STDIN_FILENO, ipipe.reader().unwrap().fd());
    act.add_file(io::STDOUT_FILENO, opipe.writer().unwrap().fd());
    let closure = wv_assert_ok!(act.run(|| {
        let hash = wv_assert_ok!(HashSession::new("hash2", &HashAlgorithm::SHAKE128));
        wv_assert_ok!(io::stdin().get_mut().hash_input(&hash, usize::MAX));
        wv_assert_ok!(io::stdout().get_mut().hash_output(&hash, PIPE_SHAKE_SIZE));
        Ok(())
    }));

    // Close unused parts of pipe that were delegated to activity
    ipipe.close_reader();
    opipe.close_writer();

    let hash = wv_assert_ok!(HashSession::new("hash1", &HashAlgorithm::SHA3_256));
    {
        // echo "Pipe!"
        let mut ifile = wv_assert_some!(ipipe.writer());
        wv_assert_ok!(writeln!(ifile, "Pipe!"));
        ipipe.close_writer();
    }
    {
        // hashsum sha3-224
        let mut ofile = wv_assert_some!(opipe.reader());
        wv_assert_ok!(ofile.hash_input(&hash, usize::MAX));
        opipe.close_reader();
    }

    let mut buf = [0u8; HashAlgorithm::SHA3_256.output_bytes];
    wv_assert_ok!(hash.finish(&mut buf));
    wv_assert_eq!(
        t,
        &buf,
        &hex!("dd20e9da838d0643a6d0e8af3ebbcac44692a32d595acd626e993dca02620aee")
    );
    wv_assert_eq!(t, closure.wait(), Ok(Code::Success));
}
