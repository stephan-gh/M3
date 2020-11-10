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

    /// Checks if the word at `index` is 255. If `false` is returned, some bit is 0 within this
    /// word.
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

    #[allow(dead_code)]
    pub fn print(&self) {
        for (idx, byte) in self.bytes.iter().enumerate() {
            println!("[{}]\t {:b}\t:{}", idx, byte, byte);
        }
    }
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
        // no dir but maybe a base left
        (0..base_start - 1, base_start..path.len())
    }
}
