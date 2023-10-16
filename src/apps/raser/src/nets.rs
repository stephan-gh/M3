/*
 * Copyright (C) 2023-2024, Stephan Gerhold <stephan@gerhold.net>
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

use crate::rotc::RoTCRawCertificate;
use m3::client::Network;
use m3::errors::{Code, Error};
use m3::io::LogFlags;
use m3::log;
use m3::net::{Endpoint, Socket, StreamSocket, StreamSocketArgs, TcpSocket};
use m3::serialize::Serialize;
use m3::time::TimeInstant;
use m3::vfs::FileRef;
use m3::{format, println};
use rot::ed25519::{Signer, SigningKey};
use rot::Hex;

const TCP_PORT: u16 = 4242;
const MAX_CHALLENGE_SIZE: usize = 512;

#[derive(Serialize, Debug)]
#[serde(crate = "m3::serde", tag = "type", rename = "challenge")]
struct ChallengePayload<'a> {
    challenge: &'a str,
    #[serde(serialize_with = "netep::serialize")]
    from: Endpoint,
}

mod netep {
    use m3::net::Endpoint;
    use m3::serialize::Serializer;

    pub fn serialize<S: Serializer>(ep: &Endpoint, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(ep)
    }
}

fn parse_challenge(mut c: &str) -> &str {
    c = c.trim();

    // Special handling for HTTP requests
    if let Some(line) = c.lines().next() {
        let mut parts = line.split(' ');
        if let (Some(method), Some(path)) = (parts.next(), parts.next()) {
            if method == "GET" && path.starts_with('/') {
                return path.trim_matches('/');
            }
        }
    }
    c
}

fn handle_client(
    socket: &mut FileRef<TcpSocket>,
    sig_key: &SigningKey,
    rotc_cert: &RoTCRawCertificate,
) -> Result<(), Error> {
    let ep = socket.accept()?;
    log!(LogFlags::Info, "Accepted remote endpoint {}", ep);

    socket.send(format!("Hello {}! I'm RASer. I like driving fast!\n", ep).as_bytes())?;
    socket.send("State your challenge: ".as_bytes())?;

    let mut buffer = [0u8; MAX_CHALLENGE_SIZE];
    let size = socket.recv(&mut buffer)?;
    let challenge = parse_challenge(
        core::str::from_utf8(&buffer[..size]).map_err(|_| Error::new(Code::Utf8Error))?,
    );
    let payload = ChallengePayload {
        challenge,
        from: ep,
    };
    log!(LogFlags::Info, "Signing challenge: {:?}", payload);
    socket.send(format!("Very well. Signing challenge '{}'...\n", payload.challenge).as_bytes())?;

    let start = TimeInstant::now();
    let raw_payload = rot::json::value::to_raw_value(&payload).unwrap();
    let signature = sig_key.sign(raw_payload.get().as_bytes());
    let cert = rot::cert::Certificate {
        payload: raw_payload,
        signature: Hex(signature.to_bytes()),
        pub_key: Hex(sig_key.verifying_key().to_bytes()),
        parent: rotc_cert,
    };
    let mut json = rot::json::to_string_pretty(&cert).unwrap();
    let elapsed = start.elapsed();
    println!("{}", json);
    json.push('\n');
    socket.send(json.as_bytes())?;
    socket.send(format!("I spent {:?} generating this. Goodbye.\n", elapsed).as_bytes())?;
    socket.close()
}

pub fn serve(sig_key: &SigningKey, rotc_cert: &RoTCRawCertificate) -> Result<(), Error> {
    let net = Network::new("net")
        .inspect_err(|e| log!(LogFlags::Error, "Setting up network failed: {}", e))?;
    let mut tcp_socket =
        TcpSocket::new(StreamSocketArgs::new(net)).expect("creating TCP socket failed");

    loop {
        tcp_socket.close().expect("close failed");
        tcp_socket.listen(TCP_PORT).expect("listen failed");
        {
            let ep = tcp_socket.local_endpoint().unwrap();
            log!(
                LogFlags::Info,
                "Listening. Feel free to connect with 'nc {0} {1}' or HTTP/0.9 to 'http://{0}:{1}/<challenge>'",
                ep.addr,
                ep.port,
            );
        }

        if let Err(e) = handle_client(&mut tcp_socket, sig_key, rotc_cert) {
            log!(LogFlags::Error, "Failed to handle client: {}", e);
        }
    }
}
