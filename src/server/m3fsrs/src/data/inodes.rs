use crate::buffer::Buffer;
use crate::internal::{
    Extent, ExtentCache, ExtentRef, FileMode, INodeRef, InodeNo, INODE_DIR_COUNT, NUM_EXT_BYTES,
    NUM_INODE_BYTES,
};

use m3::{
    cap::Selector,
    com::Perm,
    errors::{Code, Error},
    math, time,
    vfs::SeekMode,
};

pub fn create(mode: FileMode) -> Result<INodeRef, Error> {
    log!(crate::LOG_INODES, "inodes::create(mode={:o})", mode);

    let ino = crate::hdl().inodes().alloc(None)?;
    let inode = get(ino)?;
    // Reset inode
    inode.as_mut().reset();
    inode.as_mut().inode = ino;
    inode.as_mut().devno = 0; /* TODO (was also in C++ todo)*/
    inode.as_mut().mode = mode;
    mark_dirty(ino);
    Ok(inode)
}

pub fn free(inode_no: InodeNo) -> Result<(), Error> {
    log!(crate::LOG_INODES, "inodes::free(inode_no={})", inode_no);

    let ino = get(inode_no)?;
    let inodeno = ino.inode as usize;
    truncate(&ino, 0, 0)?;
    crate::hdl().inodes().free(inodeno, 1)
}

pub fn get(inode: InodeNo) -> Result<INodeRef, Error> {
    log!(crate::LOG_INODES, "inodes::get({})", inode);

    let inos_per_block = crate::hdl().superblock().inodes_per_block();
    let bno = crate::hdl().superblock().first_inode_block() + (inode / inos_per_block as u32);
    let block = crate::hdl().metabuffer().get_block(bno, false)?;

    // Calc the byte offset of this inode within its block
    let offset = (inode as usize % inos_per_block as usize) * NUM_INODE_BYTES as usize;
    Ok(INodeRef::from_buffer(block, offset))
}

pub fn seek(
    inode: &INodeRef,
    off: &mut usize,
    whence: SeekMode,
    extent: &mut usize,
    extoff: &mut usize,
) -> Result<usize, Error> {
    log!(
        crate::LOG_INODES,
        "inodes::seek(inode={}, off={}, whence={}, extent={}, extoff={})",
        inode.inode,
        off,
        whence,
        extent,
        extoff
    );

    assert!(whence != SeekMode::CUR);

    let blocksize = crate::hdl().superblock().block_size;
    let mut indir = None;

    // seeking to the end
    if whence == SeekMode::END {
        // TODO support off != 0
        assert!(*off == 0);

        *extent = inode.extents as usize;
        *extoff = 0;

        // determine extent offset
        if *extent > 0 {
            let ext = get_extent(inode, *extent - 1, &mut indir, false)?;
            *extoff = (ext.length * blocksize) as usize;
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

    // Since we don't want to find just the end, go through the extents until we found
    // the extent that contains the `off`
    let mut pos = 0;
    for i in 0..inode.extents {
        let ext = get_extent(inode, i as usize, &mut indir, false)?;

        if *off < (ext.length * blocksize) as usize {
            *extent = i as usize;
            *extoff = *off;
            return Ok(pos);
        }

        pos += (ext.length * blocksize) as usize;
        *off -= (ext.length * blocksize) as usize;
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
    accessed: usize,
) -> Result<usize, Error> {
    log!(
        crate::LOG_INODES,
        "inodes::get_extent_mem(inode={}, extent={}, extoff={}, extlen={})",
        inode.inode,
        extent,
        extoff,
        extlen
    );

    let mut indir = None;
    let ext = get_extent(inode, extent, &mut indir, false)?;
    if ext.length == 0 {
        *extlen = 0;
        return Ok(0);
    }

    // Create memory capability for extent
    let blocksize = crate::hdl().superblock().block_size;
    *extlen = (ext.length * blocksize) as usize;

    let mut bytes = crate::hdl()
        .backend()
        .get_filedata(*ext, extoff, perms, sel, true, accessed)?;

    // Stop at file end
    if (extent == (inode.extents - 1) as usize)
        && ((ext.length * blocksize) as usize <= (extoff + bytes))
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
    accessed: usize,
) -> Result<(usize, Option<Extent>), Error> {
    let num_extents = inode.extents;

    log!(
        crate::LOG_INODES,
        "inodes::req_append(inode={}, i={}, extoff={}, extlen={}, accessed={}, num_extents={})",
        inode.inode,
        i,
        extoff,
        extlen,
        accessed,
        num_extents
    );

    let mut load = true;

    if i < inode.extents as usize {
        let mut indir = None;
        let ext = &mut get_extent(inode, i, &mut indir, false)?;

        *extlen = (ext.length * crate::hdl().superblock().block_size) as usize;
        let bytes = crate::hdl()
            .backend()
            .get_filedata(**ext, extoff, perm, sel, load, accessed)?;
        Ok((bytes, None))
    }
    else {
        let ext = create_extent(None, crate::hdl().extend() as u32, accessed)?;

        if !crate::hdl().clear_blocks() {
            load = false;
        }

        extoff = 0;
        *extlen = (ext.length * crate::hdl().superblock().block_size) as usize;
        let bytes = crate::hdl()
            .backend()
            .get_filedata(ext, extoff, perm, sel, load, accessed)?;
        Ok((bytes, Some(ext)))
    }
}

pub fn append_extent(inode: &INodeRef, next: Extent, newext: &mut bool) -> Result<(), Error> {
    log!(
        crate::LOG_INODES,
        "inodes::append_extent(inode={}, next=(start={}, length={}), newext={})",
        inode.inode,
        next.start,
        next.length,
        newext
    );

    let mut indir = None;
    *newext = true;

    // try to load existing inode
    let ext = if inode.extents > 0 {
        let ext = get_extent(inode, (inode.extents - 1) as usize, &mut indir, false)?;
        if ext.start + ext.length != next.start {
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

    // if found, append to extent
    if let Some(ref ext) = ext {
        ext.as_mut().length += next.length;
    }
    // create new extent
    else {
        let ext = get_extent(inode, inode.extents as usize, &mut indir, true)?;
        inode.as_mut().extents += 1;

        *ext.as_mut() = next;
    }

    Ok(())
}

pub fn get_extent(
    inode: &INodeRef,
    mut i: usize,
    indir: &mut Option<ExtentCache>,
    create: bool,
) -> Result<ExtentRef, Error> {
    log!(
        crate::LOG_INODES,
        "inodes::get_extent(inode={}, i={}, create={})",
        inode.inode,
        i,
        create
    );

    if i < INODE_DIR_COUNT {
        // i is still within the direct array of the inode
        return Ok(ExtentRef::dir_from_inode(&inode, i));
    }
    i -= INODE_DIR_COUNT;

    let mb = crate::hdl().metabuffer();

    // Try to find/put the searched ext within the inodes indirect block
    if i < crate::hdl().superblock().extents_per_block() {
        // Create indirect block if not done yet
        if indir.is_none() {
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

            // load block and initialize extents
            let mut data_ref = mb.get_block(inode.indirect, false)?;
            if created {
                data_ref.overwrite_zero();
            }

            // create extent cache from that block
            *indir = Some(ExtentCache::from_buffer(data_ref));
        }

        // Accessing 0 should be save, otherwise the indirect block would be loaded before
        if create && indir.as_ref().unwrap()[0].length == 0 {
            crate::hdl().metabuffer().mark_dirty(inode.indirect);
        }

        return Ok(indir.as_ref().unwrap().get_ref(i)); // Finally return loaded i-th indirect extent
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
            crate::LOG_INODES,
            "Using d-indirect block, WARNING: not fully tested atm."
        );

        // Create indirect block if necessary
        created = false;

        // TODO Not sure here. The C++ code uses the data_ref pointer and increments by i/ext_per_block. However, this could then be some not align_of(Extent) byte
        // within the indirect block.
        // I changed it not to be the (i/ext_perblock)-th extent within this block
        let dind_ext_pointer = ExtentRef::indir_from_buffer(
            dind_block_ref,
            (i / crate::hdl().superblock().extents_per_block()) * NUM_EXT_BYTES,
        );

        // Create the indirect block at dint_ext_pointer.start if needed.
        if dind_ext_pointer.length == 0 {
            crate::hdl().metabuffer().mark_dirty(inode.dindirect);
            dind_ext_pointer.as_mut().start = crate::hdl().blocks().alloc(None)?;
            dind_ext_pointer.as_mut().length = 1;
            created = true;
        }
        // init with zeros
        let mut ind_block_ref = mb.get_block(dind_ext_pointer.start, false)?;
        if created {
            ind_block_ref.overwrite_zero();
        }

        // Finally get extent and return
        let ext = ExtentRef::indir_from_buffer(
            ind_block_ref,
            (i % crate::hdl().superblock().extents_per_block()) * NUM_EXT_BYTES,
        );

        if create && ext.length == 0 {
            crate::hdl().metabuffer().mark_dirty(dind_ext_pointer.start);
        }

        return Ok(ext);
    }

    // i was not even within the d extent
    Err(Error::new(Code::NotFound))
}

fn change_extent(
    inode: &INodeRef,
    mut i: usize,
    indir: &mut Option<ExtentCache>,
    remove: bool,
) -> Result<ExtentRef, Error> {
    log!(
        crate::LOG_INODES,
        "inodes::change_extent(inode={}, i={}, remove={})",
        inode.inode,
        i,
        remove,
    );

    let ext_per_block = crate::hdl().superblock().extents_per_block();

    if i < INODE_DIR_COUNT {
        return Ok(ExtentRef::dir_from_inode(&inode, i));
    }

    i -= INODE_DIR_COUNT;

    let mb = crate::hdl().metabuffer();

    if i < ext_per_block {
        assert!(inode.indirect != 0);

        if indir.is_none() {
            // indir is allocated but not laoded in the indir-vec. Do that now
            let data_ref = mb.get_block(inode.indirect, false)?;
            // Overwrite extent vec with the created one
            *indir = Some(ExtentCache::from_buffer(data_ref));
        }
        crate::hdl().metabuffer().mark_dirty(inode.indirect);

        // we assume that we only delete extents at the end; thus, if its the first, we can remove
        // the indirect block as well.
        if remove && i == 0 {
            crate::hdl().blocks().free(inode.indirect as usize, 1)?;
            inode.as_mut().indirect = 0;
        }

        // Return i-th inode from the loaded indirect block
        return Ok(indir.as_ref().unwrap().get_ref(i));
    }

    i -= ext_per_block;
    if i < (ext_per_block * ext_per_block) {
        assert!(inode.dindirect != 0);

        // Load dindirect into vec
        let data_ref = mb.get_block(inode.indirect, false)?;

        let ptr = ExtentRef::indir_from_buffer(data_ref, (i / ext_per_block) * NUM_EXT_BYTES);

        let dindir = mb.get_block(ptr.start, false)?;

        let ext_loc = (i % ext_per_block) * NUM_EXT_BYTES;
        let ext = ExtentRef::indir_from_buffer(dindir, ext_loc);

        crate::hdl().metabuffer().mark_dirty(ptr.start);

        // same here: if its the first, remove the indirect, an maybe the indirect block
        if remove {
            // Is first block in dind block
            if ext_loc == 0 {
                crate::hdl().blocks().free(ptr.start as usize, 1)?;
                ptr.as_mut().start = 0;
                ptr.as_mut().length = 0;
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

pub fn create_extent(
    inode: Option<&INodeRef>,
    blocks: u32,
    accessed: usize,
) -> Result<Extent, Error> {
    let mut count = blocks as usize;
    let start = crate::hdl().blocks().alloc(Some(&mut count))?;
    let ext = Extent::new(start, count as u32);

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

        mark_dirty(ino.inode);
    }

    Ok(ext)
}

pub fn truncate(inode: &INodeRef, extent: usize, extoff: usize) -> Result<(), Error> {
    log!(
        crate::LOG_INODES,
        "inodes::truncate(inode={}, extent={}, extoff={})",
        inode.inode,
        extent,
        extoff
    );

    let blocksize = crate::hdl().superblock().block_size;
    let mut indir = None;

    let iextents: usize = inode.extents as usize;

    if iextents > 0 {
        // erase everything up to `extent`
        let mut i = iextents - 1;
        while i > extent {
            let ext = change_extent(inode, i, &mut indir, true)?;
            crate::hdl()
                .blocks()
                .free(ext.start as usize, ext.length as usize)?;
            inode.as_mut().extents -= 1;
            inode.as_mut().size -= (ext.length * blocksize) as u64;
            ext.as_mut().start = 0;
            ext.as_mut().length = 0;
            i -= 1;
        }

        // get `extent` and determine length
        let ext = change_extent(inode, extent, &mut indir, extoff == 0)?;
        if ext.length > 0 {
            let mut curlen = ext.length * blocksize;

            let modul = inode.size % blocksize as u64;
            if modul != 0 {
                curlen -= blocksize - modul as u32;
            }

            // do we need to reduce the size of `extent` ?
            if extoff < curlen as usize {
                let diff = curlen as usize - extoff;
                let bdiff = if extoff == 0 {
                    math::round_up(diff, blocksize as usize)
                }
                else {
                    diff
                };
                let blocks = bdiff / blocksize as usize;
                if blocks > 0 {
                    // Free all of these blocks
                    crate::hdl()
                        .blocks()
                        .free((ext.start + ext.length) as usize - blocks, blocks)?;
                }
                inode.as_mut().size -= diff as u64;
                ext.as_mut().length = (ext.length as usize - blocks) as u32;
                if ext.length == 0 {
                    ext.as_mut().start = 0;
                    inode.as_mut().extents -= 1;
                }
            }
        }
        mark_dirty(inode.inode);
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
    log!(
        crate::LOG_INODES,
        "inodes::sync_metadata(inode={})",
        inode.inode,
    );

    let mut indir = None;
    for ext_idx in 0..inode.extents {
        // Load extent from inode
        let extent = get_extent(inode, ext_idx as usize, &mut indir, false)?;

        for block in extent.blocks() {
            if crate::hdl().metabuffer().dirty(block) {
                crate::hdl().backend().sync_meta(block)?;
            }
        }
    }
    Ok(())
}
