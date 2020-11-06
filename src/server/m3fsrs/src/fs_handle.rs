use crate::buffer::Buffer;
use crate::internal::BlockNo;
use crate::meta_buffer::MetaBufferHead;
use crate::sess::request::Request;
use crate::{
    backend::Backend, data::allocator::Allocator, file_buffer::FileBuffer, internal::SuperBlock,
    meta_buffer::MetaBuffer, sess::open_files::OpenFiles, FsSettings,
};

use m3::cell::{Ref, RefCell, RefMut};
use m3::col::String;
use m3::errors::Error;
use m3::rc::Rc;

/// Handle to the real file system based on some backend
pub struct M3FSHandle {
    backend: Rc<RefCell<dyn Backend + 'static>>,
    settings: FsSettings<'static>,

    super_block: SuperBlock,
    file_buffer: Rc<RefCell<FileBuffer>>,
    meta_buffer: Rc<RefCell<MetaBuffer>>,

    blocks: Rc<RefCell<Allocator>>,
    inodes: Rc<RefCell<Allocator>>,

    files: Rc<RefCell<OpenFiles>>,
}

impl M3FSHandle {
    pub fn new<B>(mut backend: B, settings: FsSettings<'static>) -> Self
    where
        B: Backend + 'static,
    {
        // Load suoerblock, then print all the superblock infor for debugginh

        log!(crate::LOG_DEF, "M3FS: Loading superblock");
        let sb = backend.load_sb().expect("Unable to load super block");
        sb.log();

        let blocks_allocator = Allocator::new(
            String::from("Block"),
            sb.first_blockbm_block(),
            sb.first_free_block.clone(),
            sb.free_blocks.clone(),
            sb.total_blocks,
            sb.blockbm_blocks(),
            sb.block_size as usize,
        );
        let inodes_allocator = Allocator::new(
            String::from("INodes"),
            sb.first_inodebm_block(),
            sb.first_free_inode.clone(),
            sb.free_inodes.clone(),
            sb.total_inodes,
            sb.inodebm_block(),
            sb.block_size as usize,
        );

        let fs_handle = M3FSHandle {
            backend: Rc::new(RefCell::new(backend)),
            file_buffer: Rc::new(RefCell::new(FileBuffer::new(sb.block_size as usize))),
            meta_buffer: Rc::new(RefCell::new(MetaBuffer::new(sb.block_size as usize))),
            settings,
            super_block: sb,

            files: Rc::new(RefCell::new(OpenFiles::new())),

            blocks: Rc::new(RefCell::new(blocks_allocator)),
            inodes: Rc::new(RefCell::new(inodes_allocator)),
        };

        fs_handle
    }

    pub fn backend<'a>(&'a self) -> Ref<'a, dyn Backend> {
        self.backend.borrow()
    }

    pub fn superblock(&self) -> &SuperBlock {
        &self.super_block
    }

    pub fn inodes<'a>(&'a self) -> RefMut<'a, Allocator> {
        self.inodes.borrow_mut()
    }

    pub fn blocks<'a>(&'a self) -> RefMut<'a, Allocator> {
        self.blocks.borrow_mut()
    }

    pub fn files<'a>(&'a self) -> RefMut<'a, OpenFiles> {
        self.files.borrow_mut()
    }

    pub fn revoke_first(&self) -> bool {
        self.settings.revoke_first
    }

    pub fn clear_blocks(&self) -> bool {
        self.settings.clear
    }

    pub fn extend(&self) -> usize {
        self.settings.extend
    }

    pub fn flush_buffer(&self) -> Result<(), Error> {
        self.meta_buffer.borrow_mut().flush()?;
        self.file_buffer.borrow_mut().flush()?;
        self.backend.borrow_mut().store_sb(&self.super_block)
    }

    pub fn metabuffer<'a>(&'a self) -> RefMut<'a, MetaBuffer> {
        self.meta_buffer.borrow_mut()
    }

    pub fn filebuffer<'a>(&'a self) -> RefMut<'a, FileBuffer> {
        self.file_buffer.borrow_mut()
    }

    pub fn get_meta_block(
        &self,
        req: &mut Request,
        bno: BlockNo,
        dirty: bool,
    ) -> Result<Rc<RefCell<MetaBufferHead>>, Error> {
        self.meta_buffer.borrow_mut().get_block(req, bno, dirty)
    }
}
