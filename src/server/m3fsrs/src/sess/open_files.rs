use crate::data::inodes;
use crate::internal::InodeNo;
use crate::FileSession;

use m3::{
    cell::RefCell,
    col::{Treap, Vec},
    errors::Error,
    rc::Rc,
};

pub struct OpenFile {
    appending: bool,
    deleted: bool,
    sessions: Vec<Rc<RefCell<FileSession>>>,
}

impl OpenFile {
    pub fn new() -> Self {
        OpenFile {
            appending: false,
            deleted: false,
            sessions: Vec::new(),
        }
    }

    pub fn appending(&self) -> bool {
        self.appending
    }

    pub fn set_appending(&mut self, new: bool) {
        self.appending = new;
    }
}

pub struct OpenFiles {
    files: Treap<InodeNo, OpenFile>,
}

impl OpenFiles {
    pub fn new() -> Self {
        OpenFiles {
            files: Treap::new(),
        }
    }

    pub fn get_file(&self, inode_num: InodeNo) -> Option<&OpenFile> {
        self.files.get(&inode_num)
    }

    pub fn get_file_mut(&mut self, inode_num: InodeNo) -> Option<&mut OpenFile> {
        self.files.get_mut(&inode_num)
    }

    pub fn delete_file(&mut self, inode_num: InodeNo) -> Result<(), Error> {
        // Create a request which executes the delete request on the FShandle
        if let Some(file) = self.get_file_mut(inode_num) {
            file.deleted = true;
        }
        else {
            inodes::free(inode_num)?;
        }
        Ok(())
    }

    pub fn add_sess(&mut self, session: Rc<RefCell<FileSession>>) {
        let session_ino = session.borrow().ino();

        if self.get_file(session.borrow().ino()).is_none() {
            let file = OpenFile::new();
            self.files.insert(session_ino, file);
        }

        self.get_file_mut(session_ino)
            .unwrap()
            .sessions
            .push(session);
    }

    pub fn remove_session(&mut self, session: Rc<RefCell<FileSession>>) -> Result<(), Error> {
        let file = self.get_file_mut(session.borrow().ino()).unwrap();

        // Search for this pointer in vec and remove when found
        let mut rm_idx = None;
        for (i, p) in file.sessions.iter().enumerate() {
            if Rc::ptr_eq(p, &session) {
                rm_idx = Some(i);
                break;
            }
        }

        let idx = rm_idx.unwrap();
        file.sessions.remove(idx);

        // If no session own this file anymore, remove it
        if file.sessions.is_empty() {
            let removed_file = self.files.remove(&session.borrow().ino());
            // unwrap save since the first line of the function would otherwise fail
            if removed_file.unwrap().deleted {
                inodes::free(session.borrow().ino())?;
            }
        }
        Ok(())
    }
}
