use smoltcp::wire::IpEndpoint;

///Formats a IpEndpoint into an m3 (IpAddr, u16) tuple.
///Assumes that the IpEndpoint a is Ipv4 address, otherwise this will panic.
pub fn to_m3_addr(addr: IpEndpoint) -> (m3::net::IpAddr, u16) {
    assert!(addr.addr.as_bytes().len() == 4, "Address was not ipv4!");
    let bytes = addr.addr.as_bytes();
    (
        m3::net::IpAddr::new(bytes[0], bytes[1], bytes[2], bytes[3]),
        addr.port,
    )
}
