use crate::buffer::Buffer;
use crate::data::*;
use crate::internal::*;
use m3::errors::{Code, Error};

pub struct Links {}

impl Links {
    pub fn create(dir: LoadedInode, name: &str, inode: LoadedInode) -> Result<(), Error> {
        log!(
            crate::LOG_DEF,
            "Links::create(dir={}, name={}, inode={})",
            { dir.inode().inode },
            name,
            { inode.inode().inode }
        ); // {} needed because of packed inode struct
        let mut indir = vec![];

        let mut created = false;

        'search_loop: for ext_idx in 0..dir.inode().extents {
            let ext = INodes::get_extent(dir.clone(), ext_idx as usize, &mut indir, false)?;

            for bno in ext.into_iter() {
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
                        new_entry.nodeno = inode.inode().inode;
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
            let ext =
                INodes::get_extent(dir.clone(), dir.inode().extents as usize, &mut indir, true)?;

            // Insert one block extent
            INodes::fill_extent(Some(dir), &ext, 1, 1)?;

            // put entry at the beginning of the block
            let start = *ext.start();
            let mut block = crate::hdl().metabuffer().get_block(start, true)?;
            let new_entry = DirEntry::from_buffer_mut(&mut block, 0);
            new_entry.set_name(name);
            new_entry.nodeno = inode.inode().inode;
            new_entry.next = crate::hdl().superblock().block_size;
        }

        inode.inode_mut().links += 1;
        INodes::mark_dirty(inode.inode().inode);
        Ok(())
    }

    pub fn remove(dir: LoadedInode, name: &str, is_dir: bool) -> Result<(), Error> {
        let mut indir = vec![];

        log!(
            crate::LOG_DEF,
            "links::remove(name={}, is_dir={})",
            name,
            is_dir
        );
        for ext_idx in 0..dir.inode().extents {
            let ext = INodes::get_extent(dir.clone(), ext_idx as usize, &mut indir, false)?;
            for bno in ext.into_iter() {
                let mut block = crate::hdl().metabuffer().get_block(bno, true)?;

                let mut prev_off = 0;
                let mut off = 0;
                let end = crate::hdl().superblock().block_size as usize;
                while off < end {
                    let entry = DirEntry::from_buffer_mut(&mut block, off);

                    if entry.name() == name {
                        // if we are not removing a dir, we are coming from unlink(). in this case, directories
                        // are not allowed
                        let inode = INodes::get(entry.nodeno)?;
                        if !is_dir && crate::internal::is_dir(inode.inode().mode) {
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
                        if (inode.inode().links - 1) == 0 {
                            let ino = inode.inode().inode;
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
}
