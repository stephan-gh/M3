use crate::buffer::Buffer;
use crate::internal::BlockNo;
use crate::meta_buffer::MetaBufferHead;
use crate::sess::request::Request;
use crate::{
    backend::Backend, data::allocator::Allocator, file_buffer::FileBuffer, internal::SuperBlock,
    meta_buffer::MetaBuffer, sess::open_files::OpenFiles, FsSettings,
};

use m3::boxed::Box;
use m3::col::String;
use m3::errors::Error;

/// Handle to the real file system based on some backend
pub struct M3FSHandle {
    backend: Box<dyn Backend + 'static>,
    settings: FsSettings<'static>,

    super_block: SuperBlock,
    file_buffer: FileBuffer,
    meta_buffer: MetaBuffer,

    blocks: Allocator,
    inodes: Allocator,

    files: OpenFiles,
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
            backend: Box::new(backend),
            file_buffer: FileBuffer::new(sb.block_size as usize),
            meta_buffer: MetaBuffer::new(sb.block_size as usize),
            settings,
            super_block: sb,

            files: OpenFiles::new(),

            blocks: blocks_allocator,
            inodes: inodes_allocator,
        };

        fs_handle
    }

    pub fn backend(&self) -> &dyn Backend {
        &*self.backend
    }

    pub fn superblock(&self) -> &SuperBlock {
        &self.super_block
    }

    pub fn inodes(&mut self) -> &mut Allocator {
        &mut self.inodes
    }

    pub fn blocks(&mut self) -> &mut Allocator {
        &mut self.blocks
    }

    pub fn files(&mut self) -> &mut OpenFiles {
        &mut self.files
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

    pub fn flush_buffer(&mut self) -> Result<(), Error> {
        self.meta_buffer.flush()?;
        self.file_buffer.flush()?;
        self.backend.store_sb(&self.super_block)
    }

    pub fn metabuffer(&mut self) -> &mut MetaBuffer {
        &mut self.meta_buffer
    }

    pub fn filebuffer(&mut self) -> &mut FileBuffer {
        &mut self.file_buffer
    }

    pub fn get_meta_block(
        &mut self,
        req: &mut Request,
        bno: BlockNo,
        dirty: bool,
    ) -> Result<&mut MetaBufferHead, Error> {
        self.meta_buffer.get_block(req, bno, dirty)
    }
}
