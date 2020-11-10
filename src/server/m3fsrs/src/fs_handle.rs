use crate::backend::Backend;
use crate::buf::{Buffer, FileBuffer, MetaBuffer};
use crate::data::{Allocator, SuperBlock};
use crate::sess::OpenFiles;
use crate::FsSettings;

use m3::boxed::Box;
use m3::col::String;
use m3::errors::Error;

/// Handle with global data structures needed at various places
pub struct M3FSHandle {
    backend: Box<dyn Backend + 'static>,
    settings: FsSettings,

    super_block: SuperBlock,
    file_buffer: FileBuffer,
    meta_buffer: MetaBuffer,

    blocks: Allocator,
    inodes: Allocator,

    files: OpenFiles,
}

impl M3FSHandle {
    pub fn new<B>(mut backend: B, settings: FsSettings) -> Self
    where
        B: Backend + 'static,
    {
        let sb = backend.load_sb().expect("Unable to load super block");
        log!(crate::LOG_DEF, "Loaded {:#?}", sb);

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

        // update superblock and write it back to disk/memory
        self.super_block
            .update_inodebm(self.inodes.free_count(), self.inodes.first_free());
        self.super_block
            .update_blockbm(self.blocks.free_count(), self.blocks.first_free());
        self.super_block.checksum = self.super_block.get_checksum();
        self.backend.store_sb(&self.super_block)
    }

    pub fn metabuffer(&mut self) -> &mut MetaBuffer {
        &mut self.meta_buffer
    }

    pub fn filebuffer(&mut self) -> &mut FileBuffer {
        &mut self.file_buffer
    }
}
