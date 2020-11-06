use crate::buffer::Buffer;
use crate::internal::NUM_INODE_BYTES;
use crate::internal::*;
use crate::sess::*;
use m3::{cap::Selector, col::Vec, com::Perm, errors::*, time};

pub struct INodes;

impl INodes {
    pub fn create(req: &mut Request, mode: Mode) -> Result<LoadedInode, Error> {
        log!(crate::LOG_DEF, "Inodes::create(mode: {:b})", mode);

        let ino = crate::hdl().inodes().alloc(req, None)?;
        let inode = INodes::get(req, ino)?;
        // Reset inode
        inode.inode().reset();
        inode.inode().inode = ino;
        inode.inode().devno = 0; /* TODO (was also in C++ todo)*/
        inode.inode().mode = mode;
        INodes::mark_dirty(req, ino);
        Ok(inode)
    }

    pub fn free(req: &mut Request, inode_no: InodeNo) -> Result<(), Error> {
        let ino = INodes::get(req, inode_no)?;
        let inodeno = ino.inode().inode as usize;
        INodes::truncate(req, ino.clone(), 0, 0)?;
        crate::hdl().inodes().free(req, inodeno, 1)
    }

    pub fn get(req: &mut Request, inode: InodeNo) -> Result<LoadedInode, Error> {
        log!(crate::LOG_DEF, "Getting inode={}", inode);

        let inos_per_block = crate::hdl().superblock().inodes_per_block();
        let bno = crate::hdl().superblock().first_inode_block() + (inode / inos_per_block as u32);
        let inodes_block = crate::hdl().metabuffer().get_block(req, bno, false)?;

        // Calc the byte offset of this inode within its block
        let inode_offset = (inode as usize % inos_per_block as usize) * NUM_INODE_BYTES as usize;
        assert!(
            inode_offset + NUM_INODE_BYTES <= crate::hdl().superblock().block_size as usize,
            "Inode exceeds block: offset+inode={}, block_size={}",
            inode_offset + NUM_INODE_BYTES,
            crate::hdl().superblock().block_size
        );
        Ok(LoadedInode::from_buffer_location(
            inodes_block,
            inode_offset,
        ))
    }

    pub fn stat(_req: &mut Request, inode: LoadedInode, info: &mut FileInfo) {
        inode.inode().to_file_info(info);
    }

    pub fn seek(
        req: &mut Request,
        inode: LoadedInode,
        off: &mut usize,
        whence: i32,
        extent: &mut usize,
        extoff: &mut usize,
    ) -> Result<usize, Error> {
        log!(
            crate::LOG_DEF,
            "Inode::seek(inode={}, off={}, whence={}, extent={}, extoff={})",
            { inode.inode().inode },
            off,
            whence,
            extent,
            extoff
        ); // {} needed because of packed inode struct

        assert!(
            whence != M3FS_SEEK_CUR,
            "INodes::seek().whence should not be M3FS_SEEK_CUR"
        );
        let mut indir = vec![];
        let blocksize = crate::hdl().superblock().block_size;

        // seeking to the end
        if whence == M3FS_SEEK_END {
            // TODO support off != 0, carried over from c++
            assert!(
                *off == 0,
                "INodes::seek() offset of != 0 is currently not supported."
            );
            *extent = inode.inode().extents as usize;
            *extoff = 0;
            // determine extent offset
            if *extent > 0 {
                let ext = INodes::get_extent(req, inode.clone(), *extent - 1, &mut indir, false)?;
                *extoff = (*ext.length() * blocksize) as usize;
                // ensure to stay within a block
                let unaligned = inode.inode().size % blocksize as u64;
                if unaligned > 0 {
                    *extoff -= (blocksize as u64 - unaligned) as usize;
                }
            }
            if *extoff > 0 {
                *extent -= 1;
            }
            *off = 0;
            return Ok(inode.inode().size as usize);
        }

        if *off as u64 > inode.inode().size {
            *off = inode.inode().size as usize;
        }
        let mut pos = 0;
        // Since we don't want to find just the end, go through the extents until we found
        // the extent that contains the `off`
        for i in 0..inode.inode().extents {
            let ext = INodes::get_extent(req, inode.clone(), i as usize, &mut indir, false)?;
            if *off < (*ext.length() * blocksize) as usize {
                *extent = i as usize;
                *extoff = *off;
                return Ok(pos);
            }
            pos += (*ext.length() * blocksize) as usize;
            *off -= (*ext.length() * blocksize) as usize;
        }
        *extent = inode.inode().extents as usize;
        *extoff = *off;
        Ok(pos)
    }

    pub fn get_extent_mem(
        req: &mut Request,
        inode: LoadedInode,
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
            { inode.inode().inode },
            extent,
            extoff,
            extlen
        ); // {} needed because of packed inode struct

        let mut indir = vec![];
        let mut ext = INodes::get_extent(req, inode.clone(), extent, &mut indir, false)?;
        if *ext.length() == 0 {
            *extlen = 0;
            return Ok(0);
        }

        // Create memory capability for extent
        let blocksize = crate::hdl().superblock().block_size;
        *extlen = (*ext.length() * blocksize) as usize;

        let mut bytes = crate::hdl()
            .backend()
            .get_filedata(req, &mut ext, extoff, perms, sel, dirty, true, accessed)?;

        // Stop at file end
        if (extent == (inode.inode().extents - 1) as usize)
            && ((*ext.length() * blocksize) as usize <= (extoff + bytes))
        {
            let rem = (inode.inode().size % blocksize as u64) as u32;
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
        req: &mut Request,
        inode: LoadedInode,
        i: usize,
        mut extoff: usize,
        extlen: &mut usize,
        sel: Selector,
        perm: Perm,
        ext: &mut LoadedExtent,
        accessed: usize,
    ) -> Result<usize, Error> {
        let num_extents = inode.inode().extents;

        log!(
            crate::LOG_DEF,
            "Inode::req_append(inode={}, i={}, extoff={}, extlen={}, accessed={}, num_extents={})",
            { inode.inode().inode },
            i,
            extoff,
            extlen,
            accessed,
            num_extents
        );

        let mut load = true;

        let ext = ext;

        if i < inode.inode().extents as usize {
            let mut indir = vec![];
            let ext = &mut INodes::get_extent(req, inode, i, &mut indir, false)?;

            *extlen = (*ext.length() * crate::hdl().superblock().block_size) as usize;
            crate::hdl()
                .backend()
                .get_filedata(req, ext, extoff, perm, sel, true, load, accessed)
        }
        else {
            INodes::fill_extent(req, None, ext, crate::hdl().extend() as u32, accessed)?;

            if !crate::hdl().clear_blocks() {
                load = false;
            }

            extoff = 0;
            *extlen = (*ext.length() * crate::hdl().superblock().block_size) as usize;
            crate::hdl()
                .backend()
                .get_filedata(req, ext, extoff, perm, sel, true, load, accessed)
        }
    }

    pub fn append_extent(
        req: &mut Request,
        inode: LoadedInode,
        next: &LoadedExtent,
        newext: &mut bool,
    ) -> Result<(), Error> {
        let mut indir = vec![];

        log!(
            crate::LOG_DEF,
            "Inodes::append_extent(inode={}, next=(start={}, length={}), newext={})",
            { inode.inode().inode },
            *next.start(),
            *next.length(),
            newext
        ); // {} needed because of packed inode struct

        *newext = true;
        // Try to load present
        let mut ext = if inode.inode().extents > 0 {
            let ext = INodes::get_extent(
                req,
                inode.clone(),
                (inode.inode().extents - 1) as usize,
                &mut indir,
                false,
            )?;
            if (*ext.start() + *ext.length()) != *next.start() {
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
                req,
                inode.clone(),
                inode.inode().extents as usize,
                &mut indir,
                true,
            )?);
            *ext.clone().unwrap().start_mut() = *next.start();
            inode.inode().extents += 1;
        }

        *ext.unwrap().length_mut() += *next.length();
        Ok(())
    }

    pub fn get_extent(
        req: &mut Request,
        inode: LoadedInode,
        mut i: usize,
        indir: &mut Vec<LoadedExtent>,
        create: bool,
    ) -> Result<LoadedExtent, Error> {
        log!(
            crate::LOG_DEF,
            "INode::get_extent(inode={}, i={}, create={})",
            { inode.inode().inode },
            i,
            create
        ); // {} needed because of packed inode struct

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
                if inode.inode().indirect == 0 {
                    // No indirect loaded, but we also should not allocate
                    if !create {
                        return Err(Error::new(Code::NotFound));
                    }
                    // Alloc block for indirect extents and put in inode.
                    let indirect_block = crate::hdl().blocks().alloc(req, None)?;
                    inode.inode().indirect = indirect_block;
                    created = true;
                }
                // Overwrite passed indirect arg with the loaded indirect block

                // Currently I initialize the block with all extents. This is a little more expensive then storing the pointers
                // in C++. However, it is also a little more save since I store the original pointer to the MetaBufferHead in the Loaded Extent.
                // Based on this pointer I can (later) savely say if there are still references active before destroying the pointer.

                let num_ext_per_block = crate::hdl().superblock().extents_per_block();
                let mut extent_vec = Vec::with_capacity(num_ext_per_block);
                let data_ref = mb.get_block(req, inode.inode().indirect, false)?;

                if created {
                    data_ref.overwrite_zero();
                }

                for j in 0..num_ext_per_block {
                    extent_vec.push(LoadedExtent::ind_from_buffer_location(
                        data_ref,
                        j * crate::internal::NUM_EXT_BYTES,
                    ));
                }
                // Overwrite extent vec with the created one
                *indir = extent_vec;
            }
            // Accessing 0 should be save, otherwise the indirect block would be loaded before
            if create && *indir[0].length() == 0 {
                crate::hdl().metabuffer().mark_dirty(inode.inode().indirect);
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
            if inode.inode().dindirect == 0 {
                if !create {
                    return Err(Error::new(Code::NotFound));
                }
                let dindirect_block = crate::hdl().blocks().alloc(req, None)?;
                inode.inode().dindirect = dindirect_block;
                created = true;
            }

            // init with zeros
            let dind_block_ref = mb.get_block(req, inode.inode().dindirect, false)?;
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
            if *dind_ext_pointer.length() == 0 {
                crate::hdl()
                    .metabuffer()
                    .mark_dirty(inode.inode().dindirect);
                *dind_ext_pointer.start_mut() = crate::hdl().blocks().alloc(req, None)?;
                *dind_ext_pointer.length_mut() = 1;
                created = true;
            }
            // init with zeros
            let ind_block_ref = mb.get_block(req, *dind_ext_pointer.start(), false)?;
            if created {
                ind_block_ref.overwrite_zero();
            }

            // Finally get extent and return
            let ext = LoadedExtent::ind_from_buffer_location(
                ind_block_ref,
                (i % crate::hdl().superblock().extents_per_block()) * NUM_EXT_BYTES,
            );

            if create && *ext.length() == 0 {
                crate::hdl()
                    .metabuffer()
                    .mark_dirty(*dind_ext_pointer.start());
            }

            return Ok(ext);
        }
        // i was not even within the d extent
        Err(Error::new(Code::NotFound))
    }

    fn change_extent(
        req: &mut Request,
        inode: LoadedInode,
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
                inode.inode().indirect != 0,
                "Inode was not in direct, but indirect is not loaded!"
            );

            if indir.len() == 0 {
                // indir is allocated but not laoded in the indir-vec. Do that now
                let data_ref = mb.get_block(req, inode.inode().indirect, false)?;

                let mut extent_vec = Vec::with_capacity(ext_per_block);
                for j in 0..ext_per_block {
                    extent_vec.push(LoadedExtent::ind_from_buffer_location(
                        data_ref,
                        j * NUM_EXT_BYTES,
                    ));
                }
                // Overwrite extent vec with the created one
                *indir = extent_vec;
            }
            crate::hdl().metabuffer().mark_dirty(inode.inode().indirect);

            // we assume that we only delete extents at the end; thus, if its the first, we can remove
            // the indirect block as well.
            if remove && i == 0 {
                crate::hdl()
                    .blocks()
                    .free(req, inode.inode().indirect as usize, 1)?;
                inode.inode().indirect = 0;
            }
            // Return i-th inode from the loaded indirect block
            return Ok(indir[i].clone());
        }

        i -= ext_per_block;
        if i < (ext_per_block * ext_per_block) {
            assert!(
                inode.inode().dindirect != 0,
                "inode was in dindirect block, but dindirect is not allocated!"
            );
            // Load dindirect into vec
            let data_ref = mb.get_block(req, inode.inode().indirect, false)?;

            let ptr = LoadedExtent::ind_from_buffer_location(
                data_ref,
                (i / ext_per_block) * NUM_EXT_BYTES,
            );

            let dindir = mb.get_block(req, *ptr.start(), false)?;

            let ext_loc = (i % ext_per_block) * NUM_EXT_BYTES;
            let ext = LoadedExtent::ind_from_buffer_location(dindir, ext_loc);

            crate::hdl().metabuffer().mark_dirty(*ptr.start());

            // same here: if its the first, remove the indirect, an maybe the indirect block
            if remove {
                // Is first block in dind block
                if ext_loc == 0 {
                    crate::hdl().blocks().free(req, *ptr.start() as usize, 1)?;
                    *ptr.length_mut() = 0;
                    *ptr.start_mut() = 0;
                    crate::hdl()
                        .metabuffer()
                        .mark_dirty(inode.inode().dindirect);
                }

                // for the double-indirect too
                if i == 0 {
                    crate::hdl()
                        .blocks()
                        .free(req, inode.inode().dindirect as usize, 1)?;
                    inode.inode().dindirect = 0;
                }
            }
            return Ok(ext);
        }
        // i not even in dindirect block
        Err(Error::new(Code::NotFound))
    }

    pub fn fill_extent(
        req: &mut Request,
        inode: Option<LoadedInode>,
        ext: &LoadedExtent,
        blocks: u32,
        accessed: usize,
    ) -> Result<(), Error> {
        let mut count = blocks as usize;
        match crate::hdl().blocks().alloc(req, Some(&mut count)) {
            Ok(start) => *ext.start_mut() = start,
            Err(e) => {
                *ext.length_mut() = 0;
                return Err(e);
            },
        }
        *ext.length_mut() = count as u32;

        let blocksize = crate::hdl().superblock().block_size;
        if crate::hdl().clear_blocks() {
            time::start(0xaaaa);
            crate::hdl().backend().clear_extent(req, ext, accessed)?;
            time::stop(0xaaaa);
        }

        if let Some(ino) = inode {
            let old_size = ino.inode().size;
            ino.inode().extents += 1;
            ino.inode().size = (old_size + blocksize as u64 - 1) & !(blocksize as u64 - 1);
            ino.inode().size += (count * blocksize as usize) as u64;

            INodes::mark_dirty(req, ino.inode().inode);
        }
        Ok(())
    }

    pub fn truncate(
        req: &mut Request,
        inode: LoadedInode,
        extent: usize,
        extoff: usize,
    ) -> Result<(), Error> {
        log!(
            crate::LOG_DEF,
            "Inode::truncate(inode={}, extent={}, extoff={})",
            { inode.inode().inode },
            extent,
            extoff
        ); // {} needed because of packed inode struct

        let blocksize = crate::hdl().superblock().block_size;
        let mut indir = vec![];

        let iextents: usize = inode.inode().extents as usize;

        if iextents > 0 {
            // erase everything up to `extent`
            let mut i = iextents - 1;
            while i > extent {
                let ext = INodes::change_extent(req, inode.clone(), i, &mut indir, true)?;
                crate::hdl()
                    .blocks()
                    .free(req, *ext.start() as usize, *ext.length() as usize)?;
                inode.inode().extents -= 1;
                inode.inode().size -= (*ext.length() * blocksize) as u64;
                *ext.length_mut() = 0;
                *ext.start_mut() = 0;
                i -= 1;
            }

            // get `extent` and determine length
            let ext = INodes::change_extent(req, inode.clone(), extent, &mut indir, extoff == 0)?;
            if *ext.length() > 0 {
                let mut curlen = *ext.length() * blocksize;

                let modul = inode.inode().size % blocksize as u64;
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
                        crate::hdl().blocks().free(
                            req,
                            (*ext.start() + *ext.length()) as usize - blocks,
                            blocks,
                        )?;
                    }
                    inode.inode().size -= diff as u64;
                    let new_length = (*ext.length() as usize - blocks) as u32;
                    *ext.length_mut() = new_length;
                    if *ext.length() == 0 {
                        *ext.start_mut() = 0;
                        inode.inode().extents -= 1;
                    }
                }
            }
            INodes::mark_dirty(req, inode.inode().inode);
        }

        Ok(())
    }

    pub fn mark_dirty(_req: &mut Request, ino: InodeNo) {
        let inos_per_block = crate::hdl().superblock().inodes_per_block();
        let block_no =
            crate::hdl().superblock().first_inode_block() + (ino / inos_per_block as u32);
        crate::hdl().metabuffer().mark_dirty(block_no);
    }

    pub fn sync_metadata(req: &mut Request, inode: LoadedInode) -> Result<(), Error> {
        let org_used = req.used_meta();
        for ext_idx in 0..inode.inode().extents {
            // Load extent from inode
            let mut indir = vec![];
            let extent =
                INodes::get_extent(req, inode.clone(), ext_idx as usize, &mut indir, false)?;

            for block in extent.into_iter() {
                if crate::hdl().metabuffer().dirty(block) {
                    crate::hdl().backend().sync_meta(req, block)?;
                }
            }
            req.pop_metas(req.used_meta() - org_used);
        }
        Ok(())
    }
}
