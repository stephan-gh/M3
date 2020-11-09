use crate::buffer::Buffer;
use crate::data::inodes;
use crate::internal::{DirEntry, INodeRef};

use m3::errors::{Code, Error};

pub fn create(dir: &INodeRef, name: &str, inode: &INodeRef) -> Result<(), Error> {
    log!(
        crate::LOG_LINKS,
        "links::create(dir={}, name={}, inode={})",
        dir.inode,
        name,
        inode.inode,
    );

    let mut indir = None;
    let mut created = false;

    'search_loop: for ext_idx in 0..dir.extents {
        let ext = inodes::get_extent(dir, ext_idx as usize, &mut indir, false)?;

        for bno in ext.blocks() {
            let mut block = crate::hdl().metabuffer().get_block(bno, true)?;

            let mut off = 0;
            let end = crate::hdl().superblock().block_size as usize;
            while off < end {
                let entry = DirEntry::from_buffer_mut(&mut block, off);

                let rem = entry.next - entry.size() as u32;
                // This happens if we can embed the new dir-entry between this one and the "next"
                if rem >= entry.size() as u32 {
                    // change current entry (thus, we cannot call entry_iter.next() again!)
                    entry.next = entry.size() as u32;
                    let entry_next = entry.next;
                    drop(entry);

                    // create new entry behind it
                    let new_entry =
                        DirEntry::from_buffer_mut(&mut block, off + entry_next as usize);

                    new_entry.set_name(name);
                    new_entry.nodeno = inode.inode;
                    new_entry.next = rem;

                    crate::hdl().metabuffer().mark_dirty(bno);

                    created = true;
                    break 'search_loop;
                }

                off += entry.next as usize;
            }
        }
    }

    // Check if a suitable space was found, otherwise extend directory
    if !created {
        // Create new
        let ext = inodes::get_extent(dir, dir.extents as usize, &mut indir, true)?;

        // Insert one block extent
        let ext_range = inodes::create_extent(Some(dir), 1, 1)?;
        *ext.as_mut() = ext_range;

        // put entry at the beginning of the block
        let start = ext.start;
        let mut block = crate::hdl().metabuffer().get_block(start, true)?;
        let new_entry = DirEntry::from_buffer_mut(&mut block, 0);
        new_entry.set_name(name);
        new_entry.nodeno = inode.inode;
        new_entry.next = crate::hdl().superblock().block_size;
    }

    inode.as_mut().links += 1;
    inodes::mark_dirty(inode.inode);
    Ok(())
}

pub fn remove(dir: &INodeRef, name: &str, is_dir: bool) -> Result<(), Error> {
    log!(
        crate::LOG_LINKS,
        "links::remove(dir={}, name={}, is_dir={})",
        dir.inode,
        name,
        is_dir
    );

    let mut indir = None;

    for ext_idx in 0..dir.extents {
        let ext = inodes::get_extent(dir, ext_idx as usize, &mut indir, false)?;
        for bno in ext.blocks() {
            let mut block = crate::hdl().metabuffer().get_block(bno, true)?;

            let mut prev_off = 0;
            let mut off = 0;
            let end = crate::hdl().superblock().block_size as usize;
            while off < end {
                let entry = DirEntry::from_buffer_mut(&mut block, off);

                if entry.name() == name {
                    // if we are not removing a dir, we are coming from unlink(). in this case, directories
                    // are not allowed
                    let inode = inodes::get(entry.nodeno)?;
                    if !is_dir && inode.mode.is_dir() {
                        return Err(Error::new(Code::IsDir));
                    }

                    let entry_next = entry.next;
                    drop(entry);

                    // remove entry by skipping over it
                    if off > 0 {
                        let mut prev = DirEntry::from_buffer_mut(&mut block, prev_off);
                        prev.next += entry_next;
                    }
                    else {
                        let next_off = off + entry_next as usize;
                        if next_off < end {
                            let (cur_entry, next_entry) = DirEntry::two_from_buffer_mut(
                                &mut block,
                                off,
                                off + entry_next as usize,
                            );

                            let dist = cur_entry.next;
                            // Copy data over
                            cur_entry.next = next_entry.next;
                            cur_entry.nodeno = next_entry.nodeno;

                            cur_entry.set_name(next_entry.name());
                            cur_entry.next = dist + next_entry.next;
                        }
                    }
                    crate::hdl().metabuffer().mark_dirty(bno);

                    // reduce links and free if necessary
                    if (inode.links - 1) == 0 {
                        let ino = inode.inode;
                        crate::hdl().files().delete_file(ino)?;
                    }

                    return Ok(());
                }

                prev_off = off;
                off += entry.next as usize;
            }
        }
    }

    Err(Error::new(Code::NoSuchFile))
}
