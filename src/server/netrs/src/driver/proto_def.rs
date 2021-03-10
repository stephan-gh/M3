pub use smoltcp::wire::Ipv4Address;

pub const ETH_HWADDR_LEN: usize = 6;

pub struct EthAddr{
    addr: [u8; ETH_HWADDR_LEN]
}

pub struct EthHdr{
    dest: EthAddr,
    src: EthAddr,
    ty: u16
}

pub const ETHTYPE_IP: u16 = 0x0008;

//Reuse the smoltcp address type
pub type Ip4Addr = Ipv4Address;

pub struct IpHdr{
    v_hl: u8,
    tos: u8,
    len: u16,
    id: u16,
    offset: u16,
    ttl: u8,
    proto: u8,
    chksum: u16,
    src: Ip4Addr,
    dest: Ip4Addr
}

pub const IP_PROTO_UDP: u8 = 17;
pub const IP_PROTO_TCP: u8 = 6;

pub const TCP_CHECKSUM_OFFSET: u8 = 0x10
pub const UDP_CHECKSUM_OFFSET: u8 = 0x06
