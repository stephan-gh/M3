use crate::buffer::Buffer;
use crate::internal::{
    FileMode, INodeRef, InodeNo, LoadedExtent, SeekMode, INODE_DIR_COUNT, NUM_EXT_BYTES,
    NUM_INODE_BYTES,
};
use crate::FileInfo;

use m3::{
    cap::Selector,
    col::Vec,
    com::Perm,
    errors::{Code, Error},
    time,
};

pub struct INodes;

impl INodes {
    pub fn create(mode: FileMode) -> Result<INodeRef, Error> {
        log!(crate::LOG_DEF, "Inodes::create(mode: {:o})", mode);

        let ino = crate::hdl().inodes().alloc(None)?;
        let inode = INodes::get(ino)?;
        // Reset inode
        inode.as_mut().reset();
        inode.as_mut().inode = ino;
        inode.as_mut().devno = 0; /* TODO (was also in C++ todo)*/
        inode.as_mut().mode = mode;
        INodes::mark_dirty(ino);
        Ok(inode)
    }

    pub fn free(inode_no: InodeNo) -> Result<(), Error> {
        let ino = INodes::get(inode_no)?;
        let inodeno = ino.inode as usize;
        INodes::truncate(&ino, 0, 0)?;
        crate::hdl().inodes().free(inodeno, 1)
    }

    pub fn get(inode: InodeNo) -> Result<INodeRef, Error> {
        log!(crate::LOG_DEF, "Getting inode={}", inode);

        let inos_per_block = crate::hdl().superblock().inodes_per_block();
        let bno = crate::hdl().superblock().first_inode_block() + (inode / inos_per_block as u32);
        let block = crate::hdl().metabuffer().get_block(bno, false)?;

        // Calc the byte offset of this inode within its block
        let offset = (inode as usize % inos_per_block as usize) * NUM_INODE_BYTES as usize;
        Ok(INodeRef::from_buffer(block, offset))
    }

    pub fn stat(inode: &INodeRef, info: &mut FileInfo) {
        inode.to_file_info(info);
    }

    pub fn seek(
        inode: &INodeRef,
        off: &mut usize,
        whence: SeekMode,
        extent: &mut usize,
        extoff: &mut usize,
    ) -> Result<usize, Error> {
        log!(
            crate::LOG_DEF,
            "Inode::seek(inode={}, off={}, whence={}, extent={}, extoff={})",
            inode.inode,
            off,
            whence,
            extent,
            extoff
        );

        assert!(
            whence != SeekMode::CUR,
            "INodes::seek().whence should not be M3FS_SEEK_CUR"
        );
        let mut indir = vec![];
        let blocksize = crate::hdl().superblock().block_size;

        // seeking to the end
        if whence == SeekMode::END {
            // TODO support off != 0, carried over from c++
            assert!(
                *off == 0,
                "INodes::seek() offset of != 0 is currently not supported."
            );
            *extent = inode.extents as usize;
            *extoff = 0;
            // determine extent offset
            if *extent > 0 {
                let ext = INodes::get_extent(inode, *extent - 1, &mut indir, false)?;
                *extoff = (ext.length() * blocksize) as usize;
                // ensure to stay within a block
                let unaligned = inode.size % blocksize as u64;
                if unaligned > 0 {
                    *extoff -= (blocksize as u64 - unaligned) as usize;
                }
            }
            if *extoff > 0 {
                *extent -= 1;
            }
            *off = 0;
            return Ok(inode.size as usize);
        }

        if *off as u64 > inode.size {
            *off = inode.size as usize;
        }
        let mut pos = 0;
        // Since we don't want to find just the end, go through the extents until we found
        // the extent that contains the `off`
        for i in 0..inode.extents {
            let ext = INodes::get_extent(inode, i as usize, &mut indir, false)?;
            if *off < (ext.length() * blocksize) as usize {
                *extent = i as usize;
                *extoff = *off;
                return Ok(pos);
            }
            pos += (ext.length() * blocksize) as usize;
            *off -= (ext.length() * blocksize) as usize;
        }
        *extent = inode.extents as usize;
        *extoff = *off;
        Ok(pos)
    }

    pub fn get_extent_mem(
        inode: &INodeRef,
        extent: usize,
        extoff: usize,
        extlen: &mut usize,
        perms: Perm,
        sel: Selector,
        dirty: bool,
        accessed: usize,
    ) -> Result<usize, Error> {
        log!(
            crate::LOG_DEF,
            "Inode::get_extent_mem(inode={}, extent={}, extoff={}, extlen={})",
            inode.inode,
            extent,
            extoff,
            extlen
        );

        let mut indir = vec![];
        let mut ext = INodes::get_extent(inode, extent, &mut indir, false)?;
        if ext.length() == 0 {
            *extlen = 0;
            return Ok(0);
        }

        // Create memory capability for extent
        let blocksize = crate::hdl().superblock().block_size;
        *extlen = (ext.length() * blocksize) as usize;

        let mut bytes = crate::hdl()
            .backend()
            .get_filedata(&mut ext, extoff, perms, sel, dirty, true, accessed)?;

        // Stop at file end
        if (extent == (inode.extents - 1) as usize)
            && ((ext.length() * blocksize) as usize <= (extoff + bytes))
        {
            let rem = (inode.size % blocksize as u64) as u32;
            if rem > 0 {
                bytes -= (blocksize - rem) as usize;
                *extlen -= (blocksize - rem) as usize;
            }
        }
        Ok(bytes)
    }

    /// Requests some extend in memory at `ext_off` for some `inode`. Stores the
    /// location and length in `ext` and returns the ext size
    pub fn req_append(
        inode: &INodeRef,
        i: usize,
        mut extoff: usize,
        extlen: &mut usize,
        sel: Selector,
        perm: Perm,
        ext: &mut LoadedExtent,
        accessed: usize,
    ) -> Result<usize, Error> {
        let num_extents = inode.extents;

        log!(
            crate::LOG_DEF,
            "Inode::req_append(inode={}, i={}, extoff={}, extlen={}, accessed={}, num_extents={})",
            inode.inode,
            i,
            extoff,
            extlen,
            accessed,
            num_extents
        );

        let mut load = true;

        let ext = ext;

        if i < inode.extents as usize {
            let mut indir = vec![];
            let ext = &mut INodes::get_extent(inode, i, &mut indir, false)?;

            *extlen = (ext.length() * crate::hdl().superblock().block_size) as usize;
            crate::hdl()
                .backend()
                .get_filedata(ext, extoff, perm, sel, true, load, accessed)
        }
        else {
            INodes::fill_extent(None, ext, crate::hdl().extend() as u32, accessed)?;

            if !crate::hdl().clear_blocks() {
                load = false;
            }

            extoff = 0;
            *extlen = (ext.length() * crate::hdl().superblock().block_size) as usize;
            crate::hdl()
                .backend()
                .get_filedata(ext, extoff, perm, sel, true, load, accessed)
        }
    }

    pub fn append_extent(
        inode: &INodeRef,
        next: &LoadedExtent,
        newext: &mut bool,
    ) -> Result<(), Error> {
        let mut indir = vec![];

        log!(
            crate::LOG_DEF,
            "Inodes::append_extent(inode={}, next=(start={}, length={}), newext={})",
            inode.inode,
            next.start(),
            next.length(),
            newext
        );

        *newext = true;
        // Try to load present
        let mut ext = if inode.extents > 0 {
            let ext = INodes::get_extent(
                inode,
                (inode.extents - 1) as usize,
                &mut indir,
                false,
            )?;
            if (ext.start() + ext.length()) != next.start() {
                None
            }
            else {
                *newext = false;
                Some(ext)
            }
        }
        else {
            None
        };

        // Load a new one
        if ext.is_none() {
            ext = Some(INodes::get_extent(
                inode,
                inode.extents as usize,
                &mut indir,
                true,
            )?);
            ext.as_ref().unwrap().set_start(next.start());
            inode.as_mut().extents += 1;
        }

        let nlength = ext.as_ref().unwrap().length() + next.length();
        ext.as_ref().unwrap().set_length(nlength);
        Ok(())
    }

    pub fn get_extent(
        inode: &INodeRef,
        mut i: usize,
        indir: &mut Vec<LoadedExtent>,
        create: bool,
    ) -> Result<LoadedExtent, Error> {
        log!(
            crate::LOG_DEF,
            "INode::get_extent(inode={}, i={}, create={})",
            inode.inode,
            i,
            create
        );

        if i < INODE_DIR_COUNT {
            // i is still within the direct array of the inode
            return Ok(LoadedExtent::Direct {
                inode_ref: inode.clone(),
                index: i,
            });
        }
        i -= INODE_DIR_COUNT;

        let mb = crate::hdl().metabuffer();

        // Try to find/put the searched ext within the inodes indirect block
        if i < crate::hdl().superblock().extents_per_block() {
            // Create indirect block if not done yet
            if indir.len() == 0 {
                let mut created = false;
                if inode.indirect == 0 {
                    // No indirect loaded, but we also should not allocate
                    if !create {
                        return Err(Error::new(Code::NotFound));
                    }
                    // Alloc block for indirect extents and put in inode.
                    let indirect_block = crate::hdl().blocks().alloc(None)?;
                    inode.as_mut().indirect = indirect_block;
                    created = true;
                }
                // Overwrite passed indirect arg with the loaded indirect block

                // Currently I initialize the block with all extents. This is a little more expensive then storing the pointers
                // in C++. However, it is also a little more save since I store the original pointer to the MetaBufferHead in the Loaded Extent.
                // Based on this pointer I can (later) savely say if there are still references active before destroying the pointer.

                let num_ext_per_block = crate::hdl().superblock().extents_per_block();
                let mut extent_vec = Vec::with_capacity(num_ext_per_block);
                let mut data_ref = mb.get_block(inode.indirect, false)?;

                if created {
                    data_ref.overwrite_zero();
                }

                for j in 0..num_ext_per_block {
                    extent_vec.push(LoadedExtent::ind_from_buffer_location(
                        data_ref.clone(),
                        j * crate::internal::NUM_EXT_BYTES,
                    ));
                }
                // Overwrite extent vec with the created one
                *indir = extent_vec;
            }
            // Accessing 0 should be save, otherwise the indirect block would be loaded before
            if create && indir[0].length() == 0 {
                crate::hdl().metabuffer().mark_dirty(inode.indirect);
            }
            return Ok(indir[i].clone()); // Finally return loaded i-th indirect extent
        }

        // Since i is not in direct or indirect part, check if we can load it in the double indirect block
        let ext_per_block = crate::hdl().superblock().extents_per_block();
        i -= ext_per_block;
        if i < (ext_per_block * ext_per_block) {
            // Not sure if thats correct since we create two blocks and not the square
            let mut created = false;
            // Create double indirect block if not done yet
            if inode.dindirect == 0 {
                if !create {
                    return Err(Error::new(Code::NotFound));
                }
                let dindirect_block = crate::hdl().blocks().alloc(None)?;
                inode.as_mut().dindirect = dindirect_block;
                created = true;
            }

            // init with zeros
            let mut dind_block_ref = mb.get_block(inode.dindirect, false)?;
            if created {
                dind_block_ref.overwrite_zero();
            }

            log!(
                crate::LOG_DEF,
                "Using d-indirect block, WARNING: not fully tested atm."
            );

            // Create indirect block if necessary
            created = false;

            // TODO Not sure here. The C++ code uses the data_ref pointer and increments by i/ext_per_block. However, this could then be some not align_of(Extent) byte
            // within the indirect block.
            // I changed it not to be the (i/ext_perblock)-th extent within this block
            let dind_ext_pointer = LoadedExtent::ind_from_buffer_location(
                dind_block_ref,
                (i / crate::hdl().superblock().extents_per_block()) * NUM_EXT_BYTES,
            );

            // Create the indirect block at dint_ext_pointer.start if needed.
            if dind_ext_pointer.length() == 0 {
                crate::hdl().metabuffer().mark_dirty(inode.dindirect);
                dind_ext_pointer.set_start(crate::hdl().blocks().alloc(None)?);
                dind_ext_pointer.set_length(1);
                created = true;
            }
            // init with zeros
            let mut ind_block_ref = mb.get_block(dind_ext_pointer.start(), false)?;
            if created {
                ind_block_ref.overwrite_zero();
            }

            // Finally get extent and return
            let ext = LoadedExtent::ind_from_buffer_location(
                ind_block_ref,
                (i % crate::hdl().superblock().extents_per_block()) * NUM_EXT_BYTES,
            );

            if create && ext.length() == 0 {
                crate::hdl()
                    .metabuffer()
                    .mark_dirty(dind_ext_pointer.start());
            }

            return Ok(ext);
        }
        // i was not even within the d extent
        Err(Error::new(Code::NotFound))
    }

    fn change_extent(
        inode: &INodeRef,
        mut i: usize,
        indir: &mut Vec<LoadedExtent>,
        remove: bool,
    ) -> Result<LoadedExtent, Error> {
        let ext_per_block = crate::hdl().superblock().extents_per_block();

        if i < INODE_DIR_COUNT {
            return Ok(LoadedExtent::Direct {
                inode_ref: inode.clone(),
                index: i,
            });
        }

        i -= INODE_DIR_COUNT;

        let mb = crate::hdl().metabuffer();

        if i < ext_per_block {
            assert!(
                inode.indirect != 0,
                "Inode was not in direct, but indirect is not loaded!"
            );

            if indir.len() == 0 {
                // indir is allocated but not laoded in the indir-vec. Do that now
                let data_ref = mb.get_block(inode.indirect, false)?;

                let mut extent_vec = Vec::with_capacity(ext_per_block);
                for j in 0..ext_per_block {
                    extent_vec.push(LoadedExtent::ind_from_buffer_location(
                        data_ref.clone(),
                        j * NUM_EXT_BYTES,
                    ));
                }
                // Overwrite extent vec with the created one
                *indir = extent_vec;
            }
            crate::hdl().metabuffer().mark_dirty(inode.indirect);

            // we assume that we only delete extents at the end; thus, if its the first, we can remove
            // the indirect block as well.
            if remove && i == 0 {
                crate::hdl().blocks().free(inode.indirect as usize, 1)?;
                inode.as_mut().indirect = 0;
            }
            // Return i-th inode from the loaded indirect block
            return Ok(indir[i].clone());
        }

        i -= ext_per_block;
        if i < (ext_per_block * ext_per_block) {
            assert!(
                inode.dindirect != 0,
                "inode was in dindirect block, but dindirect is not allocated!"
            );
            // Load dindirect into vec
            let data_ref = mb.get_block(inode.indirect, false)?;

            let ptr = LoadedExtent::ind_from_buffer_location(
                data_ref,
                (i / ext_per_block) * NUM_EXT_BYTES,
            );

            let dindir = mb.get_block(ptr.start(), false)?;

            let ext_loc = (i % ext_per_block) * NUM_EXT_BYTES;
            let ext = LoadedExtent::ind_from_buffer_location(dindir, ext_loc);

            crate::hdl().metabuffer().mark_dirty(ptr.start());

            // same here: if its the first, remove the indirect, an maybe the indirect block
            if remove {
                // Is first block in dind block
                if ext_loc == 0 {
                    crate::hdl().blocks().free(ptr.start() as usize, 1)?;
                    ptr.set_length(0);
                    ptr.set_start(0);
                    crate::hdl().metabuffer().mark_dirty(inode.dindirect);
                }

                // for the double-indirect too
                if i == 0 {
                    crate::hdl().blocks().free(inode.dindirect as usize, 1)?;
                    inode.as_mut().dindirect = 0;
                }
            }
            return Ok(ext);
        }
        // i not even in dindirect block
        Err(Error::new(Code::NotFound))
    }

    pub fn fill_extent(
        inode: Option<&INodeRef>,
        ext: &LoadedExtent,
        blocks: u32,
        accessed: usize,
    ) -> Result<(), Error> {
        let mut count = blocks as usize;
        match crate::hdl().blocks().alloc(Some(&mut count)) {
            Ok(start) => ext.set_start(start),
            Err(e) => {
                ext.set_length(0);
                return Err(e);
            },
        }
        ext.set_length(count as u32);

        let blocksize = crate::hdl().superblock().block_size;
        if crate::hdl().clear_blocks() {
            time::start(0xaaaa);
            crate::hdl().backend().clear_extent(ext, accessed)?;
            time::stop(0xaaaa);
        }

        if let Some(ino) = inode {
            let old_size = ino.size;
            ino.as_mut().extents += 1;
            ino.as_mut().size = (old_size + blocksize as u64 - 1) & !(blocksize as u64 - 1);
            ino.as_mut().size += (count * blocksize as usize) as u64;

            INodes::mark_dirty(ino.inode);
        }
        Ok(())
    }

    pub fn truncate(inode: &INodeRef, extent: usize, extoff: usize) -> Result<(), Error> {
        log!(
            crate::LOG_DEF,
            "Inode::truncate(inode={}, extent={}, extoff={})",
            inode.inode,
            extent,
            extoff
        );

        let blocksize = crate::hdl().superblock().block_size;
        let mut indir = vec![];

        let iextents: usize = inode.extents as usize;

        if iextents > 0 {
            // erase everything up to `extent`
            let mut i = iextents - 1;
            while i > extent {
                let ext = INodes::change_extent(inode, i, &mut indir, true)?;
                crate::hdl()
                    .blocks()
                    .free(ext.start() as usize, ext.length() as usize)?;
                inode.as_mut().extents -= 1;
                inode.as_mut().size -= (ext.length() * blocksize) as u64;
                ext.set_length(0);
                ext.set_start(0);
                i -= 1;
            }

            // get `extent` and determine length
            let ext = INodes::change_extent(inode, extent, &mut indir, extoff == 0)?;
            if ext.length() > 0 {
                let mut curlen = ext.length() * blocksize;

                let modul = inode.size % blocksize as u64;
                if modul != 0 {
                    curlen -= blocksize - modul as u32;
                }

                // do we need to reduce the size of `extent` ?
                if extoff < curlen as usize {
                    let diff = curlen as usize - extoff;
                    let bdiff = if extoff == 0 {
                        crate::util::round_up(diff, blocksize as usize)
                    }
                    else {
                        diff
                    };
                    let blocks = bdiff / blocksize as usize;
                    if blocks > 0 {
                        // Free all of these blocks
                        crate::hdl()
                            .blocks()
                            .free((ext.start() + ext.length()) as usize - blocks, blocks)?;
                    }
                    inode.as_mut().size -= diff as u64;
                    let new_length = (ext.length() as usize - blocks) as u32;
                    ext.set_length(new_length);
                    if ext.length() == 0 {
                        ext.set_start(0);
                        inode.as_mut().extents -= 1;
                    }
                }
            }
            INodes::mark_dirty(inode.inode);
        }

        Ok(())
    }

    pub fn mark_dirty(ino: InodeNo) {
        let inos_per_block = crate::hdl().superblock().inodes_per_block();
        let block_no =
            crate::hdl().superblock().first_inode_block() + (ino / inos_per_block as u32);
        crate::hdl().metabuffer().mark_dirty(block_no);
    }

    pub fn sync_metadata(inode: &INodeRef) -> Result<(), Error> {
        for ext_idx in 0..inode.extents {
            // Load extent from inode
            let mut indir = vec![];
            let extent = INodes::get_extent(inode, ext_idx as usize, &mut indir, false)?;

            for block in extent.into_iter() {
                if crate::hdl().metabuffer().dirty(block) {
                    crate::hdl().backend().sync_meta(block)?;
                }
            }
        }
        Ok(())
    }
}
