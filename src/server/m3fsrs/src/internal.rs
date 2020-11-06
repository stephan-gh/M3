use m3::cell::{Ref, RefCell, RefMut};
use m3::col::String;
use m3::libc;
use m3::rc::Rc;
use m3::util::size_of;

use crate::meta_buffer::MetaBufferHead;

use core::intrinsics;
use core::slice;
use core::u32;

/// Number of some block
pub type BlockNo = u32;

pub type Dev = u8;

pub type Mode = u32;

pub type InodeNo = u32;

pub type Time = u32;

pub const INVALID_INO: InodeNo = u32::MAX;
pub const M3FS_SEEK_SET: i32 = 0;
pub const M3FS_SEEK_CUR: i32 = 1;
pub const M3FS_SEEK_END: i32 = 2;
//                                            ugo
pub const M3FS_IFMT: u32 = 0o0160000;
pub const M3FS_IFLNK: u32 = 0o0120000;
pub const M3FS_IFPIP: u32 = 0o0110000;
pub const M3FS_IFREG: u32 = 0o0100000;
pub const M3FS_IFBLK: u32 = 0o0060000;
pub const M3FS_IFDIR: u32 = 0o0040000;
pub const M3FS_IFCHR: u32 = 0o0020000;
pub const M3FS_ISUID: u32 = 0o0004000;
pub const M3FS_ISGID: u32 = 0o0002000;
pub const M3FS_ISSTICKY: u32 = 0o0001000;
pub const M3FS_IRWXU: u32 = 0o0000700;
pub const M3FS_IRUSR: u32 = 0o0000400;
pub const M3FS_IWUSR: u32 = 0o0000200;
pub const M3FS_IXUSR: u32 = 0o0000100;
pub const M3FS_IRWXG: u32 = 0o0000070;
pub const M3FS_IRGRP: u32 = 0o0000040;
pub const M3FS_IWGRP: u32 = 0o0000020;
pub const M3FS_IXGRP: u32 = 0o0000010;
pub const M3FS_IRWXO: u32 = 0o0000007;
pub const M3FS_IROTH: u32 = 0o0000004;
pub const M3FS_IWOTH: u32 = 0o0000002;
pub const M3FS_IXOTH: u32 = 0o0000001;

pub const INODE_DIR_COUNT: usize = 3;
pub const MAX_BLOCK_SIZE: u32 = 4096;

pub fn is_dir(mode: Mode) -> bool {
    ((mode) & M3FS_IFMT) == M3FS_IFDIR
}

pub fn is_reg(mode: Mode) -> bool {
    (mode & M3FS_IFMT) == M3FS_IFREG
}

pub fn is_link(mode: Mode) -> bool {
    (mode & M3FS_IFMT) == M3FS_IFLNK
}

pub fn is_chr(mode: Mode) -> bool {
    (mode & M3FS_IFMT) == M3FS_IFCHR
}

pub fn is_blk(mode: Mode) -> bool {
    (mode & M3FS_IFMT) == M3FS_IFBLK
}

pub fn is_pip(mode: Mode) -> bool {
    (mode & M3FS_IFMT) == M3FS_IFPIP
}

pub const FILE_R: u64 = 1;
pub const FILE_W: u64 = 2;
pub const FILE_X: u64 = 4;
pub const FILE_RW: u64 = FILE_R | FILE_W;
pub const FILE_RWX: u64 = FILE_R | FILE_W | FILE_X;
pub const FILE_TRUNC: u64 = 8;
pub const FILE_APPEND: u64 = 16;
pub const FILE_CREATE: u64 = 32;
pub const FILE_NODATA: u64 = 64;

#[derive(Debug)]
pub struct FileInfo {
    pub devno: Dev,
    pub inode: InodeNo,
    pub mode: Mode,
    pub links: usize,
    pub size: usize,
    pub lastaccess: Time,
    pub lastmod: Time,
    pub blocksize: usize,
    // for debugging
    pub extents: usize,
    pub firstblock: BlockNo,
}

impl m3::serialize::Marshallable for FileInfo {
    fn marshall(&self, sink: &mut dyn m3::serialize::Sink) {
        self.devno.marshall(sink);
        self.inode.marshall(sink);
        self.mode.marshall(sink);
        self.links.marshall(sink);
        self.size.marshall(sink);
        self.lastaccess.marshall(sink);
        self.lastmod.marshall(sink);
        self.blocksize.marshall(sink);
        self.extents.marshall(sink);
        self.firstblock.marshall(sink);
    }
}

impl Default for FileInfo {
    fn default() -> Self {
        FileInfo {
            devno: 0,
            inode: 0,
            mode: 0,
            links: 0,
            size: 0,
            lastaccess: 0,
            lastmod: 0,
            blocksize: crate::hdl().superblock().block_size as usize,
            extents: 0,
            firstblock: 0,
        }
    }
}

/// In memory version of INodes as they appear on disk.
// should be 64 bytes large
#[repr(C, packed)]
pub struct INode {
    pub devno: Dev,
    pub links: u16,

    pub pad: u8, // Is this really a padding, was originally named "8"

    pub inode: InodeNo,
    pub mode: Mode,
    pub size: u64,

    pub lastaccess: Time,
    pub lastmod: Time,
    pub extents: u32,

    pub direct: [Extent; INODE_DIR_COUNT], // direct entries
    pub indirect: BlockNo,                 // location of the indirect block if != 0,
    pub dindirect: BlockNo,                // location of double indirect block if != 0
}

impl Clone for INode {
    fn clone(&self) -> Self {
        INode {
            devno: self.devno,
            links: self.links,
            pad: self.pad,

            inode: self.inode,
            mode: self.mode,
            size: self.size,

            lastaccess: self.lastaccess,
            lastmod: self.lastmod,
            extents: self.extents,

            direct: self.direct,     // direct entries
            indirect: self.indirect, // location of the indirect block if != 0,
            dindirect: self.dindirect,
        }
    }
}

impl INode {
    pub fn reset(&mut self) {
        self.devno = 0;
        self.links = 0;
        self.pad = 0;
        self.inode = 0;
        self.mode = 0;
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
        info.mode = self.mode;
        info.links = self.links as usize;
        info.size = self.size as usize;
        info.lastaccess = self.lastaccess;
        info.lastmod = self.lastmod;
        info.extents = self.extents as usize;
        info.blocksize = crate::hdl().superblock().block_size as usize;
        info.firstblock = self.direct[0].start;
    }
}

/// Represents a inode within a MetaBuffer block. Can be manipulated via the getter and setter functions, which writes out the
/// changes immediately. Internally a pointer to the source block is saved as well as the location of this inode within the block.
///
/// The wrapper around the original `Inode` is needed to safely hold several pointer to the same block for several INodes at once.
pub struct LoadedInode {
    /// Reference to the location of the Inodes data
    pub(crate) data_ref: Rc<RefCell<MetaBufferHead>>,
    // Offset into the block where the inode is located.
    pub(crate) inode_location: usize,
    pub(crate) inode: RefCell<&'static mut INode>,
}

pub const NUM_INODE_BYTES: usize = 64; // While the struct has another alignment, the data in the memory is read and writte as 64 bytes.

#[allow(dead_code)]
fn to_flags(mode: u32) -> String {
    let mut flags = vec!['-'; 10];
    if is_dir(mode) {
        flags[0] = 'd'
    }

    if mode & M3FS_IRUSR > 0 {
        flags[1] = 'r';
    }
    if mode & M3FS_IWUSR > 0 {
        flags[2] = 'w';
    }
    if mode & M3FS_IXUSR > 0 {
        flags[3] = 'x';
    }

    if mode & M3FS_IRGRP > 0 {
        flags[4] = 'r';
    }
    if mode & M3FS_IWGRP > 0 {
        flags[5] = 'w';
    }
    if mode & M3FS_IXGRP > 0 {
        flags[6] = 'x';
    }

    if mode & M3FS_IROTH > 0 {
        flags[7] = 'r';
    }
    if mode & M3FS_IWOTH > 0 {
        flags[8] = 'w';
    }
    if mode & M3FS_IXOTH > 0 {
        flags[9] = 'x';
    }

    flags.into_iter().collect()
}

impl Clone for LoadedInode {
    fn clone(&self) -> Self {
        // Creates a copied pointer but this should be okay according to safety comment in `from_buffer_location`
        LoadedInode::from_buffer_location(self.data_ref.clone(), self.inode_location)
    }
}

impl LoadedInode {
    pub fn from_buffer_location(buffer: Rc<RefCell<MetaBufferHead>>, location: usize) -> Self {
        // Use transmute to create an Inode from the buffer and load it into the variable.
        // the "LoadedInode" wrapper keeps track that the buffer is at least as long alive as the inode itself
        let inode: &'static mut INode = unsafe {
            debug_assert!(size_of::<INode>() == 64, "Inode is not 64 bytes long!");
            debug_assert!(
                (location % size_of::<INode>()) == 0,
                "Inode location {} is not multiple of Inode size",
                location
            );
            debug_assert!(
                (location + size_of::<INode>()) < buffer.borrow().data().len(),
                "Inode at location {} does not fit in buffer of size {}",
                location,
                buffer.borrow().data().len()
            );

            // Since all conditions for the cast should be checked at this point, transmute the pointer into the buffer.
            // Safety: Borrowing the same Inode several time is potentially unsafe, when sharing the pointers. However, currently
            // m3fsrs is single threaded, so it should behave like a copied pointer in C/C++ i guess.
            let inode_offset = location / size_of::<INode>();
            let mem = buffer.borrow_mut().data_mut().as_mut_ptr();
            let mut cast_ptr = mem.cast::<INode>();
            cast_ptr = cast_ptr.add(inode_offset);
            &mut *cast_ptr
        };

        LoadedInode {
            data_ref: buffer,
            inode_location: location,
            inode: RefCell::new(inode),
        }
    }

    pub fn inode(&self) -> RefMut<&'static mut INode> {
        self.inode.borrow_mut()
    }

    pub fn inode_im(&self) -> Ref<&'static mut INode> {
        self.inode.borrow()
    }

    pub fn to_file_info(&self, info: &mut FileInfo) {
        info.devno = self.inode.borrow().devno;
        info.inode = self.inode.borrow().inode;
        info.mode = self.inode.borrow().mode;
        info.links = self.inode.borrow().links as usize;
        info.size = self.inode.borrow().size as usize;
        info.lastaccess = self.inode.borrow().lastaccess;
        info.lastmod = self.inode.borrow().lastmod;
        info.extents = self.inode.borrow().extents as usize;
        info.blocksize = crate::hdl().superblock().block_size as usize;
        info.firstblock = self.inode.borrow().direct[0].start;
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

const DIR_ENTRY_LEN: usize = 12;

impl DirEntry {
    /// Returns a reference to the directory entry stored at `off` in the given buffer
    pub fn from_buffer(buffer: Rc<RefCell<MetaBufferHead>>, off: usize) -> &'static Self {
        // TODO ensure that name_length and next are within bounds (in case FS image is corrupt)
        unsafe {
            let buffer_off = buffer.borrow().data().as_ptr().add(off);
            let name_length = buffer_off.add(size_of::<InodeNo>()) as *const u32;
            let slice = [buffer_off as usize, *name_length as usize + DIR_ENTRY_LEN];
            intrinsics::transmute(slice)
        }
    }

    /// Returns a mutable reference to the directory entry stored at `off` in the given buffer
    pub fn from_buffer_mut(buffer: Rc<RefCell<MetaBufferHead>>, off: usize) -> &'static mut Self {
        // TODO ensure that name_length and next are within bounds (in case FS image is corrupt)
        unsafe {
            let buffer_off = buffer.borrow_mut().data_mut().as_mut_ptr().add(off);
            let name_length = buffer_off.add(size_of::<InodeNo>()) as *const u32;
            let slice = [buffer_off as usize, *name_length as usize + DIR_ENTRY_LEN];
            intrinsics::transmute(slice)
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

pub const NUM_EXT_BYTES: usize = 8;
#[derive(Clone, Copy, Debug)]
#[repr(C, align(8))]
pub struct Extent {
    pub start: u32,
    pub length: u32,
}

/// There are three possible sources of an Loaded extent. Either from a indirect block,
/// in that case we carry a refrence to the block as well as the location within this block of the extent.
/// Secondly the extent might be one of the INODE_DIR_COUNT directly saved extent. In that case we have to read/write
/// to the stored inode.
/// And as a third option the extent might not be saved in an inode or block at all, but just memory. In that case we use `Unstored`.
pub enum LoadedExtent {
    Indirect {
        data_ref: Rc<RefCell<MetaBufferHead>>,
        ext_location: usize,
        extent: RefCell<&'static mut Extent>,
    },
    Direct {
        inode_ref: LoadedInode,
        index: usize,
    },
    // An extent that has no Inode, currently only used in file_session::get_next_in_out.
    // might become obsolte when refactoring.
    Unstored {
        extent: Rc<RefCell<Extent>>,
    },
}

impl Clone for LoadedExtent {
    fn clone(&self) -> Self {
        match self {
            LoadedExtent::Indirect {
                data_ref,
                ext_location,
                extent: _,
            } => LoadedExtent::ind_from_buffer_location(data_ref.clone(), *ext_location),
            LoadedExtent::Direct { inode_ref, index } => LoadedExtent::Direct {
                inode_ref: inode_ref.clone(),
                index: *index,
            },
            LoadedExtent::Unstored { extent } => LoadedExtent::Unstored {
                extent: extent.clone(),
            },
        }
    }
}

impl LoadedExtent {
    /// Loads an indirect block from some MetaBufferHead
    pub fn ind_from_buffer_location(buffer: Rc<RefCell<MetaBufferHead>>, location: usize) -> Self {
        debug_assert!(
            location % size_of::<Extent>() == 0,
            "Extent location is not multiple of extent size!"
        );
        debug_assert!(
            location + size_of::<Extent>() < buffer.borrow().data().len(),
            "Extent location exceeds buffer!"
        );

        let loc = location / size_of::<Extent>();
        let ext = unsafe {
            let mem = buffer.borrow_mut().data_mut().as_mut_ptr();
            let mut cmem = mem.cast::<Extent>();
            cmem = cmem.add(loc);
            &mut *cmem
        };

        LoadedExtent::Indirect {
            data_ref: buffer,
            ext_location: location,
            extent: RefCell::new(ext),
        }
    }

    pub fn length(&self) -> Ref<u32> {
        match self {
            LoadedExtent::Indirect {
                data_ref: _,
                ext_location: _,
                extent,
            } => Ref::map(extent.borrow(), |ex| &ex.length),
            LoadedExtent::Direct { inode_ref, index } => {
                Ref::map(inode_ref.inode_im(), |i| &i.direct[*index].length)
            },
            LoadedExtent::Unstored { extent } => Ref::map(extent.borrow(), |e| &e.length),
        }
    }

    pub fn length_mut(&self) -> RefMut<u32> {
        match self {
            LoadedExtent::Indirect {
                data_ref: _,
                ext_location: _,
                extent,
            } => RefMut::map(extent.borrow_mut(), |ex| &mut ex.length),
            LoadedExtent::Direct { inode_ref, index } => {
                RefMut::map(inode_ref.inode(), |i| &mut i.direct[*index].length)
            },
            LoadedExtent::Unstored { extent } => {
                RefMut::map(extent.borrow_mut(), |e| &mut e.length)
            },
        }
    }

    pub fn start(&self) -> Ref<u32> {
        match self {
            LoadedExtent::Indirect {
                data_ref: _,
                ext_location: _,
                extent,
            } => Ref::map(extent.borrow(), |ex| &ex.start),
            LoadedExtent::Direct { inode_ref, index } => {
                Ref::map(inode_ref.inode_im(), |i| &i.direct[*index].start)
            },
            LoadedExtent::Unstored { extent } => Ref::map(extent.borrow(), |e| &e.start),
        }
    }

    pub fn start_mut(&self) -> RefMut<u32> {
        match self {
            LoadedExtent::Indirect {
                data_ref: _,
                ext_location: _,
                extent,
            } => RefMut::map(extent.borrow_mut(), |ex| &mut ex.start),
            LoadedExtent::Direct { inode_ref, index } => {
                RefMut::map(inode_ref.inode(), |i| &mut i.direct[*index].start)
            },
            LoadedExtent::Unstored { extent } => RefMut::map(extent.borrow_mut(), |e| &mut e.start),
        }
    }
}

impl IntoIterator for LoadedExtent {
    type IntoIter = ExtentBlocksIterator;
    type Item = BlockNo;

    fn into_iter(self) -> Self::IntoIter {
        ExtentBlocksIterator {
            start: *self.start(),
            j: 0,
            length: *self.length(),
        }
    }
}
pub struct ExtentBlocksIterator {
    start: u32,
    j: u32,
    length: u32,
}

impl core::iter::Iterator for ExtentBlocksIterator {
    type Item = BlockNo;

    fn next(&mut self) -> Option<Self::Item> {
        if self.j < self.length {
            let cur = self.j;
            self.j += 1;

            Some(self.start + cur)
        }
        else {
            None
        }
    }
}

/// represents how a superblock is stored on disk.
#[repr(C, align(8))]
pub struct SuperBlockStorage {
    pub block_size: u32,
    pub total_inodes: u32,
    pub total_blocks: u32,
    pub free_inodes: u32,
    pub free_blocks: u32,
    pub first_free_inode: u32,
    pub first_free_block: u32,
    pub checksum: u32,
}

impl SuperBlockStorage {
    pub fn to_superblock(self) -> SuperBlock {
        SuperBlock {
            block_size: self.block_size,
            total_inodes: self.total_inodes,
            total_blocks: self.total_blocks,
            free_inodes: Rc::new(RefCell::new(self.free_inodes)),
            free_blocks: Rc::new(RefCell::new(self.free_blocks)),
            first_free_inode: Rc::new(RefCell::new(self.first_free_inode)),
            first_free_block: Rc::new(RefCell::new(self.first_free_block)),
            checksum: Rc::new(RefCell::new(self.checksum)),
        }
    }

    pub fn empty() -> Self {
        let mut sb = SuperBlockStorage {
            block_size: 0,
            total_inodes: 0,
            total_blocks: 0,
            free_inodes: 0,
            free_blocks: 0,
            first_free_inode: 0,
            first_free_block: 0,
            checksum: 0,
        };

        let checksum = sb.get_checksum();
        sb.checksum = checksum;
        sb
    }

    fn get_checksum(&self) -> u32 {
        1 + self.block_size * 2
            + self.total_inodes * 3
            + self.total_blocks * 5
            + self.free_inodes * 7
            + self.free_blocks * 11
            + self.first_free_inode * 13
            + self.first_free_block * 17
    }
}

/// A loaded superblock, setup with smartpointers for sharing betweent the allocators.
pub struct SuperBlock {
    pub block_size: u32,
    pub total_inodes: u32,
    pub total_blocks: u32,
    pub free_inodes: Rc<RefCell<u32>>,
    pub free_blocks: Rc<RefCell<u32>>,
    pub first_free_inode: Rc<RefCell<u32>>,
    pub first_free_block: Rc<RefCell<u32>>,
    pub checksum: Rc<RefCell<u32>>,
}

impl SuperBlock {
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

    pub fn inode_blocks(&self) -> BlockNo {
        (self.total_inodes * (size_of::<INode>() as u32) + self.block_size - 1) / self.block_size
    }

    pub fn first_data_block(&self) -> BlockNo {
        self.first_inode_block() + self.inode_blocks()
    }

    pub fn extents_per_block(&self) -> usize {
        self.block_size as usize / NUM_EXT_BYTES
    }

    pub fn inodes_per_block(&self) -> usize {
        self.block_size as usize / NUM_INODE_BYTES
    }

    pub fn get_checksum(&self) -> u32 {
        1 + self.block_size * 2
            + self.total_inodes * 3
            + self.total_blocks * 5
            + *self.free_inodes.borrow() * 7
            + *self.free_blocks.borrow() * 11
            + *self.first_free_inode.borrow() * 13
            + *self.first_free_block.borrow() * 17
    }

    /// Writes info about the superblock to the log
    pub fn log(&self) {
        log!(crate::LOG_DEF, "SuperBlock: ");
        log!(crate::LOG_DEF, "    blocksize={}", self.block_size);
        log!(crate::LOG_DEF, "    total_inodes={}", self.total_inodes);
        log!(crate::LOG_DEF, "    total_blocks={}", self.total_blocks);
        log!(
            crate::LOG_DEF,
            "    free_inodes={}",
            self.free_inodes.borrow()
        );
        log!(
            crate::LOG_DEF,
            "    free_blocks={}",
            self.free_blocks.borrow()
        );
        log!(
            crate::LOG_DEF,
            "    first_free_inode={}",
            self.first_free_inode.borrow()
        );
        log!(
            crate::LOG_DEF,
            "    first_free_block={}",
            self.first_free_block.borrow()
        );
        if self.get_checksum() != *self.checksum.borrow() {
            panic!("Supberblock checksum is invalid, terminating!");
        }
    }

    pub fn to_storage(&self) -> SuperBlockStorage {
        SuperBlockStorage {
            block_size: self.block_size,
            total_inodes: self.total_inodes,
            total_blocks: self.total_blocks,
            free_inodes: *self.free_inodes.borrow(),
            free_blocks: *self.free_blocks.borrow(),
            first_free_inode: *self.first_free_inode.borrow(),
            first_free_block: *self.first_free_block.borrow(),
            checksum: *self.checksum.borrow(),
        }
    }
}
