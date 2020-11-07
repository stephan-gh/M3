use crate::internal::*;
use crate::meta_buffer::MetaBufferBlock;
use m3::cell::{Cell, RefCell};
use m3::rc::Rc;

use core::ops::Range;

/// Bitmap wrapper for a number of bytes.
pub struct Bitmap<'a> {
    bytes: &'a mut [u8],
}

impl<'a> Bitmap<'a> {
    pub fn from_bytes(bytes: &'a mut [u8]) -> Bitmap<'a> {
        Bitmap { bytes }
    }

    /// Returns the index of the first bit with the word in which `index` lays.
    pub fn get_first_index_of_word(index: usize) -> usize {
        (index / Bitmap::word_size()) * Bitmap::word_size()
    }

    /// Checks if the word at `index` is 255. If `false` is returned, some bit is 0 within this word.
    pub fn is_word_set(&self, mut index: usize) -> bool {
        index = index / Bitmap::word_size();
        self.bytes[index] == core::u8::MAX
    }

    pub fn is_word_unset(&self, mut index: usize) -> bool {
        index = index / Bitmap::word_size();
        self.bytes[index] == core::u8::MIN
    }

    pub fn set_word(&mut self, mut index: usize) {
        index = index / Bitmap::word_size();
        self.bytes[index] = core::u8::MAX;
    }

    pub fn unset_word(&mut self, mut index: usize) {
        index = index / Bitmap::word_size();
        self.bytes[index] = core::u8::MIN;
    }

    fn get_word_bit_index(index: usize) -> (usize, usize) {
        let word_index = index / Bitmap::word_size();
        let bit_index = index - Bitmap::get_first_index_of_word(index);

        (word_index, bit_index)
    }

    pub fn set_bit(&mut self, index: usize) {
        let (word_index, bit_index) = Bitmap::get_word_bit_index(index);
        self.bytes[word_index] = self.bytes[word_index] | (1 << bit_index);
    }

    pub fn unset_bit(&mut self, index: usize) {
        let (word_index, bit_index) = Bitmap::get_word_bit_index(index);
        self.bytes[word_index] = self.bytes[word_index] & (core::u8::MAX ^ (1 << bit_index));
    }

    pub fn is_bit_set(&self, index: usize) -> bool {
        let (word_index, bit_index) = Bitmap::get_word_bit_index(index);
        ((self.bytes[word_index] >> bit_index) & 1) == 1
    }

    pub fn word_size() -> usize {
        8
    }

    pub fn print(&self) {
        for (idx, byte) in self.bytes.iter().enumerate() {
            println!("[{}]\t {:b}\t:{}", idx, byte, byte);
        }
    }
}

// TODO abstract to a T maybe that implements all the ops needed.
pub fn round_up(value: usize, align: usize) -> usize {
    (value + align - 1) & !(align - 1)
}

/// takes some path, returns the next component as well as the rest path. Returns none if there is no other pattern
pub fn next_start_end<'a>(st: &'a str, last_end: usize) -> Option<(usize, usize)> {
    let mut new_start = last_end;
    // Move over all / until we found a real start
    loop {
        if let Some(ch) = st.get(new_start..new_start + 1) {
            if ch == "/" {
                new_start += 1;
            }
            else {
                break;
            }
        }
        else {
            // Indexed outside of string
            return None;
        }
    }

    let mut new_end = new_start;

    loop {
        if let Some(ch) = st.get(new_end..new_end + 1) {
            if ch == "/" {
                break;
            }
            else {
                new_end += 1;
            }
        }
        else {
            // Sampled outside, but might be just the last component, therefore move sample back one
            break;
        }
    }

    Some((new_start, new_end))
}

/// Returns the range in which range the last directory of the path is.
///
/// - get_base_dir("/foo/bar.baz") == ((0..4), (5..11))
/// - get_base_dir("/foo/bar/") == ((0..9), (10..10));
/// - get_base_dir("foo") == ((0..0, 0..2));
pub fn get_base_dir<'a>(path: &'a str) -> (Range<usize>, Range<usize>) {
    // Search from back for first /, if found, check if / is not last char of string.
    let mut base_start = path.len() - 1;
    while let Some(ch) = path.get(base_start..base_start + 1) {
        if ch == "/" {
            base_start += 1;
            break;
        }
        else {
            base_start = if let Some(new_start) = base_start.checked_sub(1) {
                new_start
            }
            else {
                return (0..0, 0..path.len());
            };
        }
    }

    if base_start < path.len() - 1 {
        (0..base_start - 1, base_start..path.len())
    }
    else {
        // No dir but maybe a base left
        (0..base_start - 1, base_start..path.len())
    }
}

/// Entry iterator takes a block and iterates over it assuming that the block contains entries.
pub struct DirEntryIterator<'e> {
    block: &'e MetaBufferBlock,
    off: Cell<usize>,
    end: usize,
}

impl<'e> DirEntryIterator<'e> {
    pub fn from_block(block: &'e MetaBufferBlock) -> Self {
        DirEntryIterator {
            block,
            off: Cell::from(0),
            end: crate::hdl().superblock().block_size as usize,
        }
    }

    /// Returns the next DirEntry
    pub fn next(&'e self) -> Option<&'e DirEntry> {
        if self.off.get() < self.end {
            let ret = DirEntry::from_buffer(self.block, self.off.get());

            self.off.set(self.off.get() + ret.next as usize);

            Some(ret)
        }
        else {
            None
        }
    }
}

pub struct LruElement<T> {
    value: T,
    next: Option<Rc<RefCell<Self>>>,
    prev: Option<Rc<RefCell<Self>>>,
}

impl<T> LruElement<T> {
    pub fn new(value: T) -> Rc<RefCell<LruElement<T>>> {
        Rc::new(RefCell::new(LruElement {
            value,
            next: None,
            prev: None,
        }))
    }

    pub fn value(&self) -> &T {
        &self.value
    }

    pub fn value_mut(&mut self) -> &mut T {
        &mut self.value
    }
}

pub struct Lru<T> {
    head: Option<Rc<RefCell<LruElement<T>>>>,
    tail: Option<Rc<RefCell<LruElement<T>>>>,
}

impl<T> Lru<T> {
    pub fn new() -> Self {
        Lru {
            head: None,
            tail: None,
        }
    }

    pub fn front(&self) -> Option<Rc<RefCell<LruElement<T>>>> {
        self.head.clone()
    }

    pub fn pop_front(&mut self) -> Option<Rc<RefCell<LruElement<T>>>> {
        if let Some(front) = self.head.take() {
            // Update front pointer
            self.head = front.borrow().next.clone();

            front.borrow_mut().next = None;
            front.borrow_mut().prev = None;
            Some(front)
        }
        else {
            None
        }
    }

    pub fn push_back(&mut self, item: Rc<RefCell<LruElement<T>>>) {
        // Have to initialize the lru
        if self.head.is_none() {
            self.head = Some(item.clone());
            self.tail = Some(item.clone());
            return;
        }

        // Has to be something, otherwise head would be uninitialized. This list can't remove items.
        item.borrow_mut().prev = self.tail.clone();
        item.borrow_mut().next = None;
        // Set old tails next pointer
        self.tail.as_ref().unwrap().borrow_mut().next = Some(item.clone());
        // Update tail pointer
        self.tail = Some(item);
    }

    pub fn move_to_back(&mut self, item: Rc<RefCell<LruElement<T>>>) {
        // No head, therefore this element cant be part and we have to move nothing
        if self.head.is_none() {
            return;
        }
        // Check if this is the head if so we only need to adjust one pointer
        if Rc::ptr_eq(&self.head.as_ref().unwrap(), &item) {
            // Update head
            self.head = item.borrow().next.clone();
        }
        // If we are moving back the tail, do nothing
        if Rc::ptr_eq(&self.tail.as_ref().unwrap(), &item) {
            return;
        }

        // Take the element out of this list. This is unsafe if `element` is not part of this list.
        if let Some(prev) = &item.borrow().prev {
            let new_next = item.borrow().next.clone();
            prev.borrow_mut().next = new_next;
        }
        if let Some(nex) = &item.borrow().next {
            let new_prev = item.borrow().prev.clone();
            nex.borrow_mut().prev = new_prev;
        }

        item.borrow_mut().next = None;
        item.borrow_mut().prev = None;

        self.push_back(item);
    }

    pub fn iter(&self) -> LruIterator<T> {
        LruIterator {
            next_element: self.head.clone(),
        }
    }
}

pub struct LruIterator<T> {
    next_element: Option<Rc<RefCell<LruElement<T>>>>,
}

impl<T> Iterator for LruIterator<T> {
    type Item = Rc<RefCell<LruElement<T>>>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(item) = self.next_element.take() {
            self.next_element = item.borrow().next.clone();
            Some(item.clone())
        }
        else {
            None
        }
    }
}
