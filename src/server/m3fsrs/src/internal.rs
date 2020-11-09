use crate::meta_buffer::{MetaBufferBlock, MetaBufferBlockRef};

use base::const_assert;

use bitflags::bitflags;

use core::intrinsics;
use core::slice;
use core::u32;

use m3::cell::Cell;
use m3::libc;
use m3::util::size_of;
use m3::vfs::FileInfo;

pub type BlockNo = u32;
pub type Dev = u8;
pub type InodeNo = u32;
pub type Time = u32;

pub const INODE_DIR_COUNT: usize = 3;
pub const MAX_BLOCK_SIZE: u32 = 4096;
pub const NUM_INODE_BYTES: usize = 64;
pub const NUM_EXT_BYTES: usize = 8;
const DIR_ENTRY_LEN: usize = 12;

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

/// Represents an INode as stored on disk.
#[repr(C)]
pub struct INode {
    pub devno: Dev,
    _pad: u8,
    pub links: u16,

    pub lastaccess: Time,
    pub lastmod: Time,
    pub extents: u32,

    pub inode: InodeNo,
    pub mode: FileMode,
    pub size: u64,

    pub direct: [Extent; INODE_DIR_COUNT], // direct entries
    pub indirect: BlockNo,                 // location of the indirect block if != 0,
    pub dindirect: BlockNo,                // location of double indirect block if != 0
}

impl Clone for INode {
    fn clone(&self) -> Self {
        const_assert!(size_of::<INode>() == NUM_INODE_BYTES);
        INode {
            devno: self.devno,
            links: self.links,
            _pad: 0,

            inode: self.inode,
            mode: self.mode,
            size: self.size,

            lastaccess: self.lastaccess,
            lastmod: self.lastmod,
            extents: self.extents,

            direct: self.direct,
            indirect: self.indirect,
            dindirect: self.dindirect,
        }
    }
}

impl INode {
    pub fn reset(&mut self) {
        self.devno = 0;
        self.links = 0;
        self.inode = 0;
        self.mode = FileMode::empty();
        self.size = 0;
        self.lastaccess = 0;
        self.lastmod = 0;
        self.extents = 0;

        self.direct = [Extent {
            start: 0,
            length: 0,
        }; INODE_DIR_COUNT];
        self.indirect = 0;
        self.dindirect = 0;
    }

    pub fn to_file_info(&self, info: &mut FileInfo) {
        info.devno = self.devno;
        info.inode = self.inode;
        info.mode = self.mode.bits() as u16;
        info.links = self.links as u32;
        info.size = self.size as usize;
        info.lastaccess = self.lastaccess;
        info.lastmod = self.lastmod;
        info.extents = self.extents as u32;
        info.blocksize = crate::hdl().superblock().block_size as u32;
        info.firstblock = self.direct[0].start;
    }
}

/// A reference to an inode within a loaded MetaBuffer block.
pub struct INodeRef {
    block_ref: MetaBufferBlockRef,
    // this pointer is valid during our lifetime, because we keep a MetaBufferBlockRef
    inode: *mut INode,
}

impl INodeRef {
    pub fn from_buffer(block_ref: MetaBufferBlockRef, off: usize) -> Self {
        let block = crate::hdl().metabuffer().get_block_by_ref(&block_ref);

        // ensure that the offset is valid
        debug_assert!(
            (off % size_of::<INode>()) == 0,
            "INode offset {} is not multiple of INode size",
            off
        );
        debug_assert!(
            (off + size_of::<INode>()) <= block.data().len(),
            "INode at offset {} not within block",
            off,
        );

        // cast to inode at that offset within the block
        // safety: if the checks above succeeded, this cast is valid
        let inode = unsafe {
            let inode_ptr = block.data_mut().as_mut_ptr().cast::<INode>();
            inode_ptr.add(off / size_of::<INode>())
        };

        Self { block_ref, inode }
    }

    pub fn as_mut(&self) -> &mut INode {
        // safety: valid because we keep a MetaBufferBlockRef
        unsafe { &mut *self.inode }
    }
}

impl core::ops::Deref for INodeRef {
    type Target = INode;

    fn deref(&self) -> &Self::Target {
        // safety: valid because we keep a MetaBufferBlockRef
        unsafe { &*self.inode }
    }
}

impl Clone for INodeRef {
    fn clone(&self) -> Self {
        Self {
            block_ref: self.block_ref.clone(),
            inode: self.inode,
        }
    }
}

/// On-disk representation of directory entries.
#[repr(packed, C)]
pub struct DirEntry {
    pub nodeno: InodeNo,
    pub name_length: u32,
    pub next: u32,
    name: [i8],
}

macro_rules! get_entry_mut {
    ($buffer_off:expr) => {{
        // TODO ensure that name_length and next are within bounds (in case FS image is corrupt)
        let name_length = $buffer_off.add(size_of::<InodeNo>()) as *const u32;
        let slice = [$buffer_off as usize, *name_length as usize + DIR_ENTRY_LEN];
        intrinsics::transmute(slice)
    }};
}

impl DirEntry {
    /// Returns a reference to the directory entry stored at `off` in the given buffer
    pub fn from_buffer(block: &MetaBufferBlock, off: usize) -> &Self {
        unsafe {
            let buffer_off = block.data().as_ptr().add(off);
            get_entry_mut!(buffer_off)
        }
    }

    /// Returns a mutable reference to the directory entry stored at `off` in the given buffer
    pub fn from_buffer_mut(block: &mut MetaBufferBlock, off: usize) -> &mut Self {
        unsafe {
            let buffer_off = block.data_mut().as_mut_ptr().add(off);
            get_entry_mut!(buffer_off)
        }
    }

    /// Returns a mutable reference to the directory entry stored at `off` in the given buffer
    pub fn two_from_buffer_mut(
        block: &mut MetaBufferBlock,
        off1: usize,
        off2: usize,
    ) -> (&mut Self, &mut Self) {
        assert!(off1 != off2);
        unsafe {
            let buffer_off1 = block.data_mut().as_mut_ptr().add(off1);
            let buffer_off2 = block.data_mut().as_mut_ptr().add(off2);
            (get_entry_mut!(buffer_off1), get_entry_mut!(buffer_off2))
        }
    }

    /// Returns the size of this entry when stored on disk. Includes the static size of the struct
    /// as well as the str. buffer size.
    pub fn size(&self) -> usize {
        DIR_ENTRY_LEN + self.name_length as usize
    }

    /// Returns the name of the entry
    pub fn name(&self) -> &str {
        unsafe {
            let sl = slice::from_raw_parts(self.name.as_ptr(), self.name_length as usize);
            &*(&sl[..sl.len()] as *const [i8] as *const str)
        }
    }

    /// Sets the name of the entry to the given one
    pub fn set_name(&mut self, name: &str) {
        self.name_length = name.len() as u32;
        unsafe {
            libc::memcpy(
                self.name.as_mut_ptr() as *mut libc::c_void,
                name.as_ptr() as *const libc::c_void,
                name.len(),
            );
        }
    }
}

/// Entry iterator takes a block and iterates over it assuming that the block contains entries.
pub struct DirEntryIterator<'e> {
    block: &'e MetaBufferBlock,
    off: Cell<usize>,
    end: usize,
}

impl<'e> DirEntryIterator<'e> {
    pub fn from_block(block: &'e MetaBufferBlock) -> Self {
        DirEntryIterator {
            block,
            off: Cell::from(0),
            end: crate::hdl().superblock().block_size as usize,
        }
    }

    /// Returns the next DirEntry
    pub fn next(&'e self) -> Option<&'e DirEntry> {
        if self.off.get() < self.end {
            let ret = DirEntry::from_buffer(self.block, self.off.get());

            self.off.set(self.off.get() + ret.next as usize);

            Some(ret)
        }
        else {
            None
        }
    }
}

/// Represents an extent as stored on disk
#[derive(Clone, Copy, Debug)]
#[repr(C, align(8))]
pub struct Extent {
    pub start: u32,
    pub length: u32,
}

impl Extent {
    pub fn new(start: u32, length: u32) -> Self {
        Self { start, length }
    }

    pub fn blocks(&self) -> core::ops::Range<u32> {
        core::ops::Range {
            start: self.start,
            end: self.start + self.length,
        }
    }
}

/// A reference to an direct or indirect extent
pub struct ExtentRef {
    block_ref: MetaBufferBlockRef,
    // this pointer is valid during our lifetime, because we keep a MetaBufferBlockRef
    extent: *mut Extent,
}

impl Clone for ExtentRef {
    fn clone(&self) -> Self {
        Self {
            block_ref: self.block_ref.clone(),
            extent: self.extent,
        }
    }
}

impl ExtentRef {
    /// Loads the extent with given index from the given INode
    pub fn dir_from_inode(inode: &INodeRef, index: usize) -> Self {
        Self {
            block_ref: inode.block_ref.clone(),
            extent: &mut inode.as_mut().direct[index],
        }
    }

    /// Loads the indirect extent at given offset from given MetaBufferBlock
    pub fn indir_from_buffer(block_ref: MetaBufferBlockRef, off: usize) -> Self {
        let block = crate::hdl().metabuffer().get_block_by_ref(&block_ref);
        debug_assert!(
            off % size_of::<Extent>() == 0,
            "Extent off is not multiple of extent size!"
        );
        debug_assert!(
            off + size_of::<Extent>() <= block.data().len(),
            "Extent off exceeds block!"
        );

        // safety: the cast is valid if the checks above succeeded
        let ext = unsafe {
            let mem = block.data_mut().as_mut_ptr();
            mem.cast::<Extent>().add(off / size_of::<Extent>())
        };

        Self {
            block_ref: block_ref.clone(),
            extent: ext,
        }
    }

    pub fn as_mut(&self) -> &mut Extent {
        // safety: valid because we keep a MetaBufferBlockRef
        unsafe { &mut *self.extent }
    }
}

impl core::ops::Deref for ExtentRef {
    type Target = Extent;

    fn deref(&self) -> &Self::Target {
        // safety: valid because we keep a MetaBufferBlockRef
        unsafe { &*self.extent }
    }
}

/// A cache for a block of extents
pub struct ExtentCache {
    block_ref: MetaBufferBlockRef,
    // this pointer is valid during our lifetime, because we keep a MetaBufferBlockRef
    extents: *const Extent,
}

impl ExtentCache {
    pub fn from_buffer(block_ref: MetaBufferBlockRef) -> Self {
        let block = crate::hdl().metabuffer().get_block_by_ref(&block_ref);
        let extents = block.data().as_ptr().cast::<Extent>();
        Self { block_ref, extents }
    }

    pub fn get_ref(&self, idx: usize) -> ExtentRef {
        ExtentRef::indir_from_buffer(self.block_ref.clone(), idx * size_of::<Extent>())
    }
}

impl core::ops::Index<usize> for ExtentCache {
    type Output = Extent;

    fn index(&self, idx: usize) -> &Self::Output {
        assert!(idx < crate::hdl().superblock().extents_per_block());
        // safety: valid because we keep a MetaBufferBlockRef
        unsafe { &*self.extents.add(idx) }
    }
}

/// Represents a superblock
#[derive(Debug)]
#[repr(C, align(8))]
pub struct SuperBlock {
    pub block_size: u32,
    pub total_inodes: u32,
    pub total_blocks: u32,
    pub free_inodes: u32,
    pub free_blocks: u32,
    pub first_free_inode: u32,
    pub first_free_block: u32,
    pub checksum: u32,
}

impl SuperBlock {
    pub fn get_checksum(&self) -> u32 {
        1 + self.block_size * 2
            + self.total_inodes * 3
            + self.total_blocks * 5
            + self.free_inodes * 7
            + self.free_blocks * 11
            + self.first_free_inode * 13
            + self.first_free_block * 17
    }

    pub fn first_inodebm_block(&self) -> BlockNo {
        1
    }

    pub fn inodebm_block(&self) -> BlockNo {
        (((self.total_inodes + 7) / 8) + self.block_size - 1) / self.block_size
    }

    pub fn first_blockbm_block(&self) -> BlockNo {
        self.first_inodebm_block() + self.inodebm_block()
    }

    pub fn blockbm_blocks(&self) -> BlockNo {
        (((self.total_blocks + 7) / 8) + self.block_size - 1) / self.block_size
    }

    pub fn first_inode_block(&self) -> BlockNo {
        self.first_blockbm_block() + self.blockbm_blocks()
    }

    pub fn extents_per_block(&self) -> usize {
        self.block_size as usize / NUM_EXT_BYTES
    }

    pub fn inodes_per_block(&self) -> usize {
        self.block_size as usize / NUM_INODE_BYTES
    }

    pub fn update_inodebm(&mut self, free: u32, first: u32) {
        self.free_inodes = free;
        self.first_free_inode = first;
    }

    pub fn update_blockbm(&mut self, free: u32, first: u32) {
        self.free_blocks = free;
        self.first_free_block = first;
    }
}
