/*
 * Copyright (C) 2021, 2023, Stephan Gerhold <stephan.gerhold@mailbox.tu-dresden.de>
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

use cshake::kmac;
use hex_literal::hex;

use m3::client::{HashInput, HashOutput, HashSession, Pipes};
use m3::col::Vec;
use m3::com::{MemCap, MemGate, Perm};
use m3::crypto::{HashAlgorithm, HashType};
use m3::errors::{Code, Error};
use m3::io;
use m3::io::{Read, Write};
use m3::mem::{GlobOff, VirtAddr};
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
    wv_run_test!(t, cshake_nist);
    wv_run_test!(t, kmac_nist);
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
    wv_assert_ok!(Activity::own()
        .pager()
        .unwrap()
        .map_mem(ADDR, mcap.sel(), SIZE, Perm::RW));

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

// echo Pipe! | hashsum shake128 -O 262144 -o - | hashsum sha3-256
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
        // hashsum sha3-256
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

struct CSHAKETest {
    algo: &'static HashAlgorithm,
    data: &'static [u8],
    n: &'static str,
    s: &'static str,
    expected: &'static [u8],
}

struct KMACTest {
    algo: &'static HashAlgorithm,
    key: &'static [u8],
    data: &'static [u8],
    output_length: usize,
    s: &'static str,
    expected: &'static [u8],
}

/// NIST test vectors for cSHAKE
/// https://csrc.nist.gov/CSRC/media/Projects/Cryptographic-Standards-and-Guidelines/documents/examples/cSHAKE_samples.pdf
const CSHAKE_NIST_SAMPLES: [CSHAKETest; 4] = [
    CSHAKETest {
        algo: &HashAlgorithm::CSHAKE128,
        data: &hex!("00 01 02 03"),
        n: "",
        s: "Email Signature",
        expected: &hex!(
            "
            C1 C3 69 25 B6 40 9A 04 F1 B5 04 FC BC A9 D8 2B
            40 17 27 7C B5 ED 2B 20 65 FC 1D 38 14 D5 AA F5
            "
        ),
    },
    CSHAKETest {
        algo: &HashAlgorithm::CSHAKE128,
        data: &hex!(
            "
            00 01 02 03 04 05 06 07 08 09 0A 0B 0C 0D 0E 0F
            10 11 12 13 14 15 16 17 18 19 1A 1B 1C 1D 1E 1F
            20 21 22 23 24 25 26 27 28 29 2A 2B 2C 2D 2E 2F
            30 31 32 33 34 35 36 37 38 39 3A 3B 3C 3D 3E 3F
            40 41 42 43 44 45 46 47 48 49 4A 4B 4C 4D 4E 4F
            50 51 52 53 54 55 56 57 58 59 5A 5B 5C 5D 5E 5F
            60 61 62 63 64 65 66 67 68 69 6A 6B 6C 6D 6E 6F
            70 71 72 73 74 75 76 77 78 79 7A 7B 7C 7D 7E 7F
            80 81 82 83 84 85 86 87 88 89 8A 8B 8C 8D 8E 8F
            90 91 92 93 94 95 96 97 98 99 9A 9B 9C 9D 9E 9F
            A0 A1 A2 A3 A4 A5 A6 A7 A8 A9 AA AB AC AD AE AF
            B0 B1 B2 B3 B4 B5 B6 B7 B8 B9 BA BB BC BD BE BF
            C0 C1 C2 C3 C4 C5 C6 C7
            "
        ),
        n: "",
        s: "Email Signature",
        expected: &hex!(
            "
            C5 22 1D 50 E4 F8 22 D9 6A 2E 88 81 A9 61 42 0F
            29 4B 7B 24 FE 3D 20 94 BA ED 2C 65 24 CC 16 6B
            "
        ),
    },
    CSHAKETest {
        algo: &HashAlgorithm::CSHAKE256,
        data: &hex!("00 01 02 03"),
        n: "",
        s: "Email Signature",
        expected: &hex!(
            "
            D0 08 82 8E 2B 80 AC 9D 22 18 FF EE 1D 07 0C 48
            B8 E4 C8 7B FF 32 C9 69 9D 5B 68 96 EE E0 ED D1
            64 02 0E 2B E0 56 08 58 D9 C0 0C 03 7E 34 A9 69
            37 C5 61 A7 4C 41 2B B4 C7 46 46 95 27 28 1C 8C
            "
        ),
    },
    CSHAKETest {
        algo: &HashAlgorithm::CSHAKE256,
        data: &hex!(
            "
            00 01 02 03 04 05 06 07 08 09 0A 0B 0C 0D 0E 0F
            10 11 12 13 14 15 16 17 18 19 1A 1B 1C 1D 1E 1F
            20 21 22 23 24 25 26 27 28 29 2A 2B 2C 2D 2E 2F
            30 31 32 33 34 35 36 37 38 39 3A 3B 3C 3D 3E 3F
            40 41 42 43 44 45 46 47 48 49 4A 4B 4C 4D 4E 4F
            50 51 52 53 54 55 56 57 58 59 5A 5B 5C 5D 5E 5F
            60 61 62 63 64 65 66 67 68 69 6A 6B 6C 6D 6E 6F
            70 71 72 73 74 75 76 77 78 79 7A 7B 7C 7D 7E 7F
            80 81 82 83 84 85 86 87 88 89 8A 8B 8C 8D 8E 8F
            90 91 92 93 94 95 96 97 98 99 9A 9B 9C 9D 9E 9F
            A0 A1 A2 A3 A4 A5 A6 A7 A8 A9 AA AB AC AD AE AF
            B0 B1 B2 B3 B4 B5 B6 B7 B8 B9 BA BB BC BD BE BF
            C0 C1 C2 C3 C4 C5 C6 C7
            "
        ),
        n: "",
        s: "Email Signature",
        expected: &hex!(
            "
            07 DC 27 B1 1E 51 FB AC 75 BC 7B 3C 1D 98 3E 8B
            4B 85 FB 1D EF AF 21 89 12 AC 86 43 02 73 09 17
            27 F4 2B 17 ED 1D F6 3E 8E C1 18 F0 4B 23 63 3C
            1D FB 15 74 C8 FB 55 CB 45 DA 8E 25 AF B0 92 BB
            "
        ),
    },
];

/// NIST test vectors for KMAC / KMACXOF
/// https://csrc.nist.gov/CSRC/media/Projects/Cryptographic-Standards-and-Guidelines/documents/examples/KMAC_samples.pdf
/// https://csrc.nist.gov/CSRC/media/Projects/Cryptographic-Standards-and-Guidelines/documents/examples/KMACXOF_samples.pdf
const KMAC_NIST_SAMPLES: [KMACTest; 12] = [
    // KMAC
    KMACTest {
        algo: &HashAlgorithm::CSHAKE128,
        key: &hex!(
            "
            40 41 42 43 44 45 46 47 48 49 4A 4B 4C 4D 4E 4F
            50 51 52 53 54 55 56 57 58 59 5A 5B 5C 5D 5E 5F
            "
        ),
        data: &hex!("00 01 02 03"),
        output_length: 256,
        s: "",
        expected: &hex!(
            "
            E5 78 0B 0D 3E A6 F7 D3 A4 29 C5 70 6A A4 3A 00
            FA DB D7 D4 96 28 83 9E 31 87 24 3F 45 6E E1 4E
            "
        ),
    },
    KMACTest {
        algo: &HashAlgorithm::CSHAKE128,
        key: &hex!(
            "
            40 41 42 43 44 45 46 47 48 49 4A 4B 4C 4D 4E 4F
            50 51 52 53 54 55 56 57 58 59 5A 5B 5C 5D 5E 5F
            "
        ),
        data: &hex!("00 01 02 03"),
        output_length: 256,
        s: "My Tagged Application",
        expected: &hex!(
            "
            3B 1F BA 96 3C D8 B0 B5 9E 8C 1A 6D 71 88 8B 71
            43 65 1A F8 BA 0A 70 70 C0 97 9E 28 11 32 4A A5
            "
        ),
    },
    KMACTest {
        algo: &HashAlgorithm::CSHAKE128,
        key: &hex!(
            "
            40 41 42 43 44 45 46 47 48 49 4A 4B 4C 4D 4E 4F
            50 51 52 53 54 55 56 57 58 59 5A 5B 5C 5D 5E 5F
            "
        ),
        data: &hex!(
            "
            00 01 02 03 04 05 06 07 08 09 0A 0B 0C 0D 0E 0F
            10 11 12 13 14 15 16 17 18 19 1A 1B 1C 1D 1E 1F
            20 21 22 23 24 25 26 27 28 29 2A 2B 2C 2D 2E 2F
            30 31 32 33 34 35 36 37 38 39 3A 3B 3C 3D 3E 3F
            40 41 42 43 44 45 46 47 48 49 4A 4B 4C 4D 4E 4F
            50 51 52 53 54 55 56 57 58 59 5A 5B 5C 5D 5E 5F
            60 61 62 63 64 65 66 67 68 69 6A 6B 6C 6D 6E 6F
            70 71 72 73 74 75 76 77 78 79 7A 7B 7C 7D 7E 7F
            80 81 82 83 84 85 86 87 88 89 8A 8B 8C 8D 8E 8F
            90 91 92 93 94 95 96 97 98 99 9A 9B 9C 9D 9E 9F
            A0 A1 A2 A3 A4 A5 A6 A7 A8 A9 AA AB AC AD AE AF
            B0 B1 B2 B3 B4 B5 B6 B7 B8 B9 BA BB BC BD BE BF
            C0 C1 C2 C3 C4 C5 C6 C7
            "
        ),
        output_length: 256,
        s: "My Tagged Application",
        expected: &hex!(
            "
            1F 5B 4E 6C CA 02 20 9E 0D CB 5C A6 35 B8 9A 15
            E2 71 EC C7 60 07 1D FD 80 5F AA 38 F9 72 92 30
            "
        ),
    },
    KMACTest {
        algo: &HashAlgorithm::CSHAKE256,
        key: &hex!(
            "
            40 41 42 43 44 45 46 47 48 49 4A 4B 4C 4D 4E 4F
            50 51 52 53 54 55 56 57 58 59 5A 5B 5C 5D 5E 5F
            "
        ),
        data: &hex!("00 01 02 03"),
        output_length: 512,
        s: "My Tagged Application",
        expected: &hex!(
            "
            20 C5 70 C3 13 46 F7 03 C9 AC 36 C6 1C 03 CB 64
            C3 97 0D 0C FC 78 7E 9B 79 59 9D 27 3A 68 D2 F7
            F6 9D 4C C3 DE 9D 10 4A 35 16 89 F2 7C F6 F5 95
            1F 01 03 F3 3F 4F 24 87 10 24 D9 C2 77 73 A8 DD
            "
        ),
    },
    KMACTest {
        algo: &HashAlgorithm::CSHAKE256,
        key: &hex!(
            "
            40 41 42 43 44 45 46 47 48 49 4A 4B 4C 4D 4E 4F
            50 51 52 53 54 55 56 57 58 59 5A 5B 5C 5D 5E 5F
            "
        ),
        data: &hex!(
            "
            00 01 02 03 04 05 06 07 08 09 0A 0B 0C 0D 0E 0F
            10 11 12 13 14 15 16 17 18 19 1A 1B 1C 1D 1E 1F
            20 21 22 23 24 25 26 27 28 29 2A 2B 2C 2D 2E 2F
            30 31 32 33 34 35 36 37 38 39 3A 3B 3C 3D 3E 3F
            40 41 42 43 44 45 46 47 48 49 4A 4B 4C 4D 4E 4F
            50 51 52 53 54 55 56 57 58 59 5A 5B 5C 5D 5E 5F
            60 61 62 63 64 65 66 67 68 69 6A 6B 6C 6D 6E 6F
            70 71 72 73 74 75 76 77 78 79 7A 7B 7C 7D 7E 7F
            80 81 82 83 84 85 86 87 88 89 8A 8B 8C 8D 8E 8F
            90 91 92 93 94 95 96 97 98 99 9A 9B 9C 9D 9E 9F
            A0 A1 A2 A3 A4 A5 A6 A7 A8 A9 AA AB AC AD AE AF
            B0 B1 B2 B3 B4 B5 B6 B7 B8 B9 BA BB BC BD BE BF
            C0 C1 C2 C3 C4 C5 C6 C7
            "
        ),
        output_length: 512,
        s: "",
        expected: &hex!(
            "
            75 35 8C F3 9E 41 49 4E 94 97 07 92 7C EE 0A F2
            0A 3F F5 53 90 4C 86 B0 8F 21 CC 41 4B CF D6 91
            58 9D 27 CF 5E 15 36 9C BB FF 8B 9A 4C 2E B1 78
            00 85 5D 02 35 FF 63 5D A8 25 33 EC 6B 75 9B 69
            "
        ),
    },
    KMACTest {
        algo: &HashAlgorithm::CSHAKE256,
        key: &hex!(
            "
            40 41 42 43 44 45 46 47 48 49 4A 4B 4C 4D 4E 4F
            50 51 52 53 54 55 56 57 58 59 5A 5B 5C 5D 5E 5F
            "
        ),
        data: &hex!(
            "
            00 01 02 03 04 05 06 07 08 09 0A 0B 0C 0D 0E 0F
            10 11 12 13 14 15 16 17 18 19 1A 1B 1C 1D 1E 1F
            20 21 22 23 24 25 26 27 28 29 2A 2B 2C 2D 2E 2F
            30 31 32 33 34 35 36 37 38 39 3A 3B 3C 3D 3E 3F
            40 41 42 43 44 45 46 47 48 49 4A 4B 4C 4D 4E 4F
            50 51 52 53 54 55 56 57 58 59 5A 5B 5C 5D 5E 5F
            60 61 62 63 64 65 66 67 68 69 6A 6B 6C 6D 6E 6F
            70 71 72 73 74 75 76 77 78 79 7A 7B 7C 7D 7E 7F
            80 81 82 83 84 85 86 87 88 89 8A 8B 8C 8D 8E 8F
            90 91 92 93 94 95 96 97 98 99 9A 9B 9C 9D 9E 9F
            A0 A1 A2 A3 A4 A5 A6 A7 A8 A9 AA AB AC AD AE AF
            B0 B1 B2 B3 B4 B5 B6 B7 B8 B9 BA BB BC BD BE BF
            C0 C1 C2 C3 C4 C5 C6 C7
            "
        ),
        output_length: 512,
        s: "My Tagged Application",
        expected: &hex!(
            "
            B5 86 18 F7 1F 92 E1 D5 6C 1B 8C 55 DD D7 CD 18
            8B 97 B4 CA 4D 99 83 1E B2 69 9A 83 7D A2 E4 D9
            70 FB AC FD E5 00 33 AE A5 85 F1 A2 70 85 10 C3
            2D 07 88 08 01 BD 18 28 98 FE 47 68 76 FC 89 65
            "
        ),
    },
    // KMACXOF
    KMACTest {
        algo: &HashAlgorithm::CSHAKE128,
        key: &hex!(
            "
            40 41 42 43 44 45 46 47 48 49 4A 4B 4C 4D 4E 4F
            50 51 52 53 54 55 56 57 58 59 5A 5B 5C 5D 5E 5F
            "
        ),
        data: &hex!("00 01 02 03"),
        output_length: kmac::XOF_OUTPUT_LENGTH,
        s: "",
        expected: &hex!(
            "
            CD 83 74 0B BD 92 CC C8 CF 03 2B 14 81 A0 F4 46
            0E 7C A9 DD 12 B0 8A 0C 40 31 17 8B AC D6 EC 35
            "
        ),
    },
    KMACTest {
        algo: &HashAlgorithm::CSHAKE128,
        key: &hex!(
            "
            40 41 42 43 44 45 46 47 48 49 4A 4B 4C 4D 4E 4F
            50 51 52 53 54 55 56 57 58 59 5A 5B 5C 5D 5E 5F
            "
        ),
        data: &hex!("00 01 02 03"),
        output_length: kmac::XOF_OUTPUT_LENGTH,
        s: "My Tagged Application",
        expected: &hex!(
            "
            31 A4 45 27 B4 ED 9F 5C 61 01 D1 1D E6 D2 6F 06
            20 AA 5C 34 1D EF 41 29 96 57 FE 9D F1 A3 B1 6C
            "
        ),
    },
    KMACTest {
        algo: &HashAlgorithm::CSHAKE128,
        key: &hex!(
            "
            40 41 42 43 44 45 46 47 48 49 4A 4B 4C 4D 4E 4F
            50 51 52 53 54 55 56 57 58 59 5A 5B 5C 5D 5E 5F
            "
        ),
        data: &hex!(
            "
            00 01 02 03 04 05 06 07 08 09 0A 0B 0C 0D 0E 0F
            10 11 12 13 14 15 16 17 18 19 1A 1B 1C 1D 1E 1F
            20 21 22 23 24 25 26 27 28 29 2A 2B 2C 2D 2E 2F
            30 31 32 33 34 35 36 37 38 39 3A 3B 3C 3D 3E 3F
            40 41 42 43 44 45 46 47 48 49 4A 4B 4C 4D 4E 4F
            50 51 52 53 54 55 56 57 58 59 5A 5B 5C 5D 5E 5F
            60 61 62 63 64 65 66 67 68 69 6A 6B 6C 6D 6E 6F
            70 71 72 73 74 75 76 77 78 79 7A 7B 7C 7D 7E 7F
            80 81 82 83 84 85 86 87 88 89 8A 8B 8C 8D 8E 8F
            90 91 92 93 94 95 96 97 98 99 9A 9B 9C 9D 9E 9F
            A0 A1 A2 A3 A4 A5 A6 A7 A8 A9 AA AB AC AD AE AF
            B0 B1 B2 B3 B4 B5 B6 B7 B8 B9 BA BB BC BD BE BF
            C0 C1 C2 C3 C4 C5 C6 C7
            "
        ),
        output_length: kmac::XOF_OUTPUT_LENGTH,
        s: "My Tagged Application",
        expected: &hex!(
            "
            47 02 6C 7C D7 93 08 4A A0 28 3C 25 3E F6 58 49
            0C 0D B6 14 38 B8 32 6F E9 BD DF 28 1B 83 AE 0F
            "
        ),
    },
    KMACTest {
        algo: &HashAlgorithm::CSHAKE256,
        key: &hex!(
            "
            40 41 42 43 44 45 46 47 48 49 4A 4B 4C 4D 4E 4F
            50 51 52 53 54 55 56 57 58 59 5A 5B 5C 5D 5E 5F
            "
        ),
        data: &hex!("00 01 02 03"),
        output_length: kmac::XOF_OUTPUT_LENGTH,
        s: "My Tagged Application",
        expected: &hex!(
            "
            17 55 13 3F 15 34 75 2A AD 07 48 F2 C7 06 FB 5C
            78 45 12 CA B8 35 CD 15 67 6B 16 C0 C6 64 7F A9
            6F AA 7A F6 34 A0 BF 8F F6 DF 39 37 4F A0 0F AD
            9A 39 E3 22 A7 C9 20 65 A6 4E B1 FB 08 01 EB 2B
            "
        ),
    },
    KMACTest {
        algo: &HashAlgorithm::CSHAKE256,
        key: &hex!(
            "
            40 41 42 43 44 45 46 47 48 49 4A 4B 4C 4D 4E 4F
            50 51 52 53 54 55 56 57 58 59 5A 5B 5C 5D 5E 5F
            "
        ),
        data: &hex!(
            "
            00 01 02 03 04 05 06 07 08 09 0A 0B 0C 0D 0E 0F
            10 11 12 13 14 15 16 17 18 19 1A 1B 1C 1D 1E 1F
            20 21 22 23 24 25 26 27 28 29 2A 2B 2C 2D 2E 2F
            30 31 32 33 34 35 36 37 38 39 3A 3B 3C 3D 3E 3F
            40 41 42 43 44 45 46 47 48 49 4A 4B 4C 4D 4E 4F
            50 51 52 53 54 55 56 57 58 59 5A 5B 5C 5D 5E 5F
            60 61 62 63 64 65 66 67 68 69 6A 6B 6C 6D 6E 6F
            70 71 72 73 74 75 76 77 78 79 7A 7B 7C 7D 7E 7F
            80 81 82 83 84 85 86 87 88 89 8A 8B 8C 8D 8E 8F
            90 91 92 93 94 95 96 97 98 99 9A 9B 9C 9D 9E 9F
            A0 A1 A2 A3 A4 A5 A6 A7 A8 A9 AA AB AC AD AE AF
            B0 B1 B2 B3 B4 B5 B6 B7 B8 B9 BA BB BC BD BE BF
            C0 C1 C2 C3 C4 C5 C6 C7
            "
        ),
        output_length: kmac::XOF_OUTPUT_LENGTH,
        s: "",
        expected: &hex!(
            "
            FF 7B 17 1F 1E 8A 2B 24 68 3E ED 37 83 0E E7 97
            53 8B A8 DC 56 3F 6D A1 E6 67 39 1A 75 ED C0 2C
            A6 33 07 9F 81 CE 12 A2 5F 45 61 5E C8 99 72 03
            1D 18 33 73 31 D2 4C EB 8F 8C A8 E6 A1 9F D9 8B
            "
        ),
    },
    KMACTest {
        algo: &HashAlgorithm::CSHAKE256,
        key: &hex!(
            "
            40 41 42 43 44 45 46 47 48 49 4A 4B 4C 4D 4E 4F
            50 51 52 53 54 55 56 57 58 59 5A 5B 5C 5D 5E 5F
            "
        ),
        data: &hex!(
            "
            00 01 02 03 04 05 06 07 08 09 0A 0B 0C 0D 0E 0F
            10 11 12 13 14 15 16 17 18 19 1A 1B 1C 1D 1E 1F
            20 21 22 23 24 25 26 27 28 29 2A 2B 2C 2D 2E 2F
            30 31 32 33 34 35 36 37 38 39 3A 3B 3C 3D 3E 3F
            40 41 42 43 44 45 46 47 48 49 4A 4B 4C 4D 4E 4F
            50 51 52 53 54 55 56 57 58 59 5A 5B 5C 5D 5E 5F
            60 61 62 63 64 65 66 67 68 69 6A 6B 6C 6D 6E 6F
            70 71 72 73 74 75 76 77 78 79 7A 7B 7C 7D 7E 7F
            80 81 82 83 84 85 86 87 88 89 8A 8B 8C 8D 8E 8F
            90 91 92 93 94 95 96 97 98 99 9A 9B 9C 9D 9E 9F
            A0 A1 A2 A3 A4 A5 A6 A7 A8 A9 AA AB AC AD AE AF
            B0 B1 B2 B3 B4 B5 B6 B7 B8 B9 BA BB BC BD BE BF
            C0 C1 C2 C3 C4 C5 C6 C7
            "
        ),
        output_length: kmac::XOF_OUTPUT_LENGTH,
        s: "My Tagged Application",
        expected: &hex!(
            "
            D5 BE 73 1C 95 4E D7 73 28 46 BB 59 DB E3 A8 E3
            0F 83 E7 7A 4B FF 44 59 F2 F1 C2 B4 EC EB B8 CE
            67 BA 01 C6 2E 8A B8 57 8D 2D 49 9B D1 BB 27 67
            68 78 11 90 02 0A 30 6A 97 DE 28 1D CC 30 30 5D
            "
        ),
    },
];

const CSHAKE_BUF_LEN: usize = HashAlgorithm::CSHAKE128.block_bytes;

fn create_cshake_sess() -> Result<HashSession, Error> {
    match HashSession::new("hash", &HashAlgorithm::CSHAKE128) {
        // ignore this test if this hash algorithm is not supported
        Err(e) if e.code() == Code::NotSup => {
            println!("Ignoring test -- CSHAKE128 not supported");
            Err(e)
        },
        Err(e) => wv_assert_ok!(Err(e)),
        Ok(sess) => Ok(sess),
    }
}

fn cshake_nist(t: &mut dyn WvTester) {
    let mut hash = match create_cshake_sess() {
        Ok(sess) => sess,
        Err(_) => return,
    };
    let mgate = wv_assert_ok!(MemGate::new(256, Perm::RW));
    let mgate_derived = wv_assert_ok!(mgate.derive_cap(0, 256, Perm::RW));
    wv_assert_ok!(hash.ep().configure(mgate_derived.sel()));
    let mut buf = [0u8; CSHAKE_BUF_LEN];

    for test in &CSHAKE_NIST_SAMPLES {
        wv_assert_ok!(hash.reset(test.algo));

        // Absorb cSHAKE header
        let size = cshake::prepend_header(&mut buf, test.n, test.s, test.algo.block_bytes);
        wv_assert_ok!(mgate.write(&buf[..size], 0));
        wv_assert_ok!(hash.input(0, size));

        // Write test data
        wv_assert_ok!(mgate.write(test.data, 0));
        wv_assert_ok!(hash.input(0, test.data.len()));

        // Check resulting hash
        wv_assert_ok!(hash.output(0, test.expected.len()));
        wv_assert_ok!(mgate.read(&mut buf[..test.expected.len()], 0));
        wv_assert_eq!(t, &buf[..test.expected.len()], test.expected);
    }
}

fn kmac_nist(t: &mut dyn WvTester) {
    let mut hash = match create_cshake_sess() {
        Ok(sess) => sess,
        Err(_) => return,
    };
    let mgate = wv_assert_ok!(MemGate::new(512, Perm::RW));
    let mgate_derived = wv_assert_ok!(mgate.derive_cap(0, 512, Perm::RW));
    wv_assert_ok!(hash.ep().configure(mgate_derived.sel()));
    let mut buf = [0u8; CSHAKE_BUF_LEN * 2]; // KMAC header and key, separately padded

    for test in &KMAC_NIST_SAMPLES {
        wv_assert_ok!(hash.reset(test.algo));

        // Absorb KMAC header and key
        let mut size = 0;
        size += kmac::prepend_header(&mut buf[size..], test.s, test.algo.block_bytes);
        size += kmac::prepend_key(&mut buf[size..], test.key, test.algo.block_bytes);
        wv_assert_ok!(mgate.write(&buf[..size], 0));
        wv_assert_ok!(hash.input(0, size));

        // Write test data and append KMAC output length
        wv_assert_ok!(mgate.write(test.data, 0));
        let appended_size = kmac::append_output_length(&mut buf, test.output_length);
        wv_assert_ok!(mgate.write(&buf[..appended_size], test.data.len() as GlobOff));
        wv_assert_ok!(hash.input(0, test.data.len() + appended_size));

        // Check resulting hash
        wv_assert_ok!(hash.output(0, test.expected.len()));
        wv_assert_ok!(mgate.read(&mut buf[..test.expected.len()], 0));
        wv_assert_eq!(t, &buf[..test.expected.len()], test.expected);
    }
}
