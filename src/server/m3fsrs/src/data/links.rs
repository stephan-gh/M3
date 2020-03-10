use crate::buffer::Buffer;
use crate::data::*;
use crate::internal::*;
use crate::sess::*;
use m3::errors::{Code, Error};

pub struct Links {}

impl Links {
    pub fn create(
        req: &mut Request,
        dir: LoadedInode,
        name: &str,
        _namelen: usize,
        inode: LoadedInode,
    ) -> Result<(), Error> {
        log!(
            crate::LOG_DEF,
            "Links::create(dir={}, name={}, inode={})",
            { dir.inode().inode },
            name,
            { inode.inode().inode }
        ); //{} needed because of packed inode struct
        let org_used = req.used_meta();
        let mut indir = vec![];

        let mut rem = 0;
        let mut search_entry = None;

        'search_loop: for ext_idx in 0..dir.inode().extents {
            let ext = INodes::get_extent(req, dir.clone(), ext_idx as usize, &mut indir, false)
                .expect("Failed to get extent for entry!");

            for bno in ext.into_iter() {
                //This is the block for all entries that are within this block
                let dir_entry_data_ref = crate::hdl().metabuffer().get_block(req, bno, false);
                let mut entry_location = 0;
                //Max offset into the buffer at which a entry could be. Is anyways incorrect since each name has a dynamic length.
                let entry_location_end = crate::hdl().superblock().block_size;
                //Iter over the entries, at the end always increment the ptr offset by entry.next
                while entry_location < entry_location_end {
                    let entry = LoadedDirEntry::from_buffer_location(
                        dir_entry_data_ref.clone(),
                        entry_location as usize,
                    );

                    rem = *entry.entry.borrow().next - entry.size() as u32;
                    //This happens if we can embed the new dir-entry between this one and the "next"
                    if rem >= entry.size() as u32 {
                        //change previous entry
                        *entry.entry.borrow_mut().next = entry.size() as u32;
                        //get pointer to new one by offseting with this entries size
                        search_entry = Some(LoadedDirEntry::from_buffer_location(
                            dir_entry_data_ref,
                            (entry_location + *entry.entry.borrow().next) as usize,
                        ));
                        crate::hdl().metabuffer().mark_dirty(bno);
                        req.pop_metas(req.used_meta() - org_used);
                        //break the loop
                        break 'search_loop;
                    }

                    //Go to next entry
                    entry_location = entry_location + *entry.entry.borrow().next;
                }
                req.pop_meta();
            }
            req.pop_metas(req.used_meta() - org_used);
        }

        //Check if a suitable space was found, otherwise extend directory
        let entry = if let Some(e) = search_entry {
            e
        }
        else {
            //Create new
            let ext = if let Some(e) = INodes::get_extent(
                req,
                dir.clone(),
                dir.inode().extents as usize,
                &mut indir,
                true,
            ) {
                e
            }
            else {
                return Err(Error::new(Code::NoSpace));
            };

            //Insert one block extent
            INodes::fill_extent(req, Some(dir), &ext, 1, 1);
            if *ext.length() == 0 {
                return Err(Error::new(Code::NoSpace));
            }

            //put entry at the beginning of the block
            rem = crate::hdl().superblock().block_size;
            let start: u32 = *ext.start();
            LoadedDirEntry::from_buffer_location(
                crate::hdl().metabuffer().get_block(req, start, true),
                0,
            )
        };

        //write entry
        //is safe since both allocation paths make sure that the needed space is given
        unsafe {
            entry.set_name(name);
        }

        *entry.entry.borrow_mut().nodeno = inode.inode().inode;
        *entry.entry.borrow_mut().next = rem;

        inode.inode().links += 1;
        INodes::mark_dirty(req, inode.inode().inode);
        Ok(())
    }

    pub fn remove(
        req: &mut Request,
        dir: LoadedInode,
        name: &str,
        name_len: usize,
        is_dir: bool,
    ) -> Result<(), Error> {
        let org_used = req.used_meta();
        let mut indir = vec![];

        log!(
            crate::LOG_DEF,
            "links::remove(name={}, is_dir={})",
            name,
            is_dir
        );
        for ext_idx in 0..dir.inode().extents {
            let ext = INodes::get_extent(req, dir.clone(), ext_idx as usize, &mut indir, false)
                .expect("Failed to get extent for entry!");
            for bno in ext.into_iter() {
                //This is the block for all entries that are within this block
                let dir_entry_data_ref = crate::hdl().metabuffer().get_block(req, bno, false);
                let mut entry_location = 0;
                //Max offset into the buffer at which a entry could be. Is anyways incorrect since each name has a dynamic length.
                let entry_location_end = crate::hdl().superblock().block_size;

                //previouse entry
                let mut prev: Option<LoadedDirEntry> = None;

                //Iter over the entries, at the end always increment the ptr offset by entry.next
                while entry_location < entry_location_end {
                    let entry = LoadedDirEntry::from_buffer_location(
                        dir_entry_data_ref.clone(),
                        entry_location as usize,
                    );

                    if *entry.entry.borrow().name_length as usize == name_len
                        && entry.entry.borrow().name == name
                    {
                        //if we are not removing a dir, we are coming from unlink(). in this case, directories
                        //are not allowed
                        let inode = INodes::get(req, *entry.entry.borrow().nodeno);
                        if !is_dir && crate::internal::is_dir(inode.inode().mode) {
                            req.pop_metas(req.used_meta() - org_used);
                            return Err(Error::new(Code::IsDir));
                        }

                        //remove entry by skipping over it
                        if let Some(p) = prev {
                            let next = *p.entry.borrow().next + *entry.entry.borrow().next;
                            *p.entry.borrow_mut().next = next;
                        }
                        else {
                            //copy the next entry back, if there is any
                            let next_location =
                                entry_location as usize + *entry.entry.borrow().next as usize;
                            let next_entry = LoadedDirEntry::from_buffer_location(
                                dir_entry_data_ref,
                                next_location,
                            );

                            if next_location < entry_location_end as usize {
                                let dist = *entry.entry.borrow().next;
                                //Copy data over
                                *entry.entry.borrow_mut().next = *next_entry.entry.borrow().next;
                                *entry.entry.borrow_mut().nodeno =
                                    *next_entry.entry.borrow().nodeno;
                                //Should be safe since we are moving "back", therefore "next" will be updated as well

                                unsafe {
                                    entry.set_name(next_entry.entry.borrow().name);
                                }
                                *entry.entry.borrow_mut().next =
                                    dist + *next_entry.entry.borrow().next;
                            }
                        }
                        crate::hdl().metabuffer().mark_dirty(bno);
                        //reduce links and free if necessary
                        if (inode.inode().links - 1) == 0 {
                            let ino = inode.inode().inode;
                            crate::hdl().files().delete_file(ino);
                        }

                        req.pop_metas(req.used_meta() - org_used);
                        return Ok(());
                    }
                    //Go to next entry
                    entry_location = entry_location + *entry.entry.borrow().next;
                    //Update pref
                    prev = Some(entry);
                }

                req.pop_meta();
            }
            req.pop_metas(req.used_meta() - org_used);
        }

        Err(Error::new(Code::NoSuchFile))
    }
}
