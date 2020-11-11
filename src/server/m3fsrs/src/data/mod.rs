mod allocator;
mod bitmap;
mod direntry;
mod extent;
mod inode;
mod superblock;

pub use allocator::Allocator;
pub use inode::INodeRef;
pub use direntry::{DirEntry, DirEntryIterator};
pub use extent::{Extent, ExtentRef, ExtentCache};
pub use superblock::SuperBlock;

use bitflags::bitflags;

pub type BlockNo = m3::session::BlockNo;
pub type BlockRange = m3::session::BlockRange;
pub type Dev = u8;
pub type InodeNo = u32;
pub type Time = u32;

pub const INODE_DIR_COUNT: usize = 3;
pub const MAX_BLOCK_SIZE: u32 = 4096;
pub const NUM_INODE_BYTES: usize = 64;
pub const NUM_EXT_BYTES: usize = 8;
pub const DIR_ENTRY_LEN: usize = 12;

bitflags! {
    pub struct FileMode : u32 {
        const IFMT      = 0o0160000;
        const IFLNK     = 0o0120000;
        const IFPIP     = 0o0110000;
        const IFREG     = 0o0100000;
        const IFBLK     = 0o0060000;
        const IFDIR     = 0o0040000;
        const IFCHR     = 0o0020000;
        const ISUID     = 0o0004000;
        const ISGID     = 0o0002000;
        const ISSTICKY  = 0o0001000;
        const IRWXU     = 0o0000700;
        const IRUSR     = 0o0000400;
        const IWUSR     = 0o0000200;
        const IXUSR     = 0o0000100;
        const IRWXG     = 0o0000070;
        const IRGRP     = 0o0000040;
        const IWGRP     = 0o0000020;
        const IXGRP     = 0o0000010;
        const IRWXO     = 0o0000007;
        const IROTH     = 0o0000004;
        const IWOTH     = 0o0000002;
        const IXOTH     = 0o0000001;

        const FILE_DEF  = Self::IFREG.bits | 0o0644;
        const DIR_DEF   = Self::IFDIR.bits;
        const PERM      = 0o777;
    }
}

#[allow(dead_code)]
impl FileMode {
    pub fn is_dir(self) -> bool {
        (self & Self::IFMT) == Self::IFDIR
    }

    pub fn is_reg(self) -> bool {
        (self & Self::IFMT) == Self::IFREG
    }

    pub fn is_link(self) -> bool {
        (self & Self::IFMT) == Self::IFLNK
    }

    pub fn is_chr(self) -> bool {
        (self & Self::IFMT) == Self::IFCHR
    }

    pub fn is_blk(self) -> bool {
        (self & Self::IFMT) == Self::IFBLK
    }

    pub fn is_pip(self) -> bool {
        (self & Self::IFMT) == Self::IFPIP
    }
}
