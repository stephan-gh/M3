/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2021 Nils Asmussen, Barkhausen Institut
 * Copyright (C) 2021 Mark Ueberall <mark.ueberall.1999@gmail.com>
 *
 * This file is part of M3 (Microkernel-based SysteM for Heterogeneous Manycores).
 *
 * M3 is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License version 2 as
 * published by the Free Software Foundation.
 *
 * M3 is distributed in the hope that it will be useful, but
 * WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
 * General Public License version 2 for more details.
 */

use crate::col::{String, Vec};
use crate::errors::{Code, Error};
use crate::serialize::{copy_str_from, str_slice_from};
use serde::de::{DeserializeSeed, EnumAccess, SeqAccess, VariantAccess, Visitor};
use serde::{Deserialize, Deserializer};

// The deserializer for recreating values from message
#[derive(Debug)]
pub struct M3Deserializer<'de> {
    slice: &'de [u64],
    pos: usize,
}

impl<'de> M3Deserializer<'de> {
    #[inline(always)]
    pub fn new(slice: &'de [u64]) -> M3Deserializer<'de> {
        M3Deserializer { slice, pos: 0 }
    }

    #[inline(always)]
    pub fn size(&self) -> usize {
        self.slice.len()
    }

    #[inline(always)]
    pub fn skip(&mut self, words: usize) {
        self.pos += words;
    }

    // retrieves an element of type T from the message slice
    #[inline(always)]
    pub fn pop<T: Deserialize<'de>>(&mut self) -> Result<T, Error> {
        T::deserialize(self)
    }

    #[inline(always)]
    fn pop_word(&mut self) -> Result<u64, Error> {
        if self.pos >= self.slice.len() {
            return Err(Error::new(Code::InvArgs));
        }

        self.pos += 1;
        Ok(self.slice[self.pos - 1])
    }

    #[inline(always)]
    fn pop_str(&mut self) -> Result<String, Error> {
        // safety: we know that the pointer and length are okay
        self.do_pop_str(|slice, pos, len| unsafe { copy_str_from(&slice[pos..], len - 1) })
    }

    #[inline(always)]
    fn pop_str_slice(&mut self) -> Result<&'static str, Error> {
        // safety: we know that the pointer and length are okay
        self.do_pop_str(|slice, pos, len| unsafe { str_slice_from(&slice[pos..], len - 1) })
    }

    #[inline(always)]
    fn pop_byte_vec(&mut self) -> Result<Vec<u8>, Error> {
        Ok(self.pop_byte_slice()?.to_vec())
    }

    #[inline(always)]
    fn pop_byte_slice(&mut self) -> Result<&'static [u8], Error> {
        // safety: we know that the pointer and length are okay
        self.do_pop_str(|slice, pos, len| unsafe {
            core::slice::from_raw_parts(slice[pos..].as_ptr() as *const u8, len)
        })
    }

    fn do_pop_str<T, F>(&mut self, f: F) -> Result<T, Error>
    where
        F: Fn(&'de [u64], usize, usize) -> T,
    {
        let len = self.pop_word()? as usize;

        let npos = self.pos + (len + 7) / 8;
        if len == 0 || npos > self.slice.len() {
            return Err(Error::new(Code::InvArgs));
        }

        let res = f(self.slice, self.pos, len);
        self.pos = npos;
        Ok(res)
    }
}

impl<'de, 'a> Deserializer<'de> for &'a mut M3Deserializer<'de> {
    type Error = Error;

    fn is_human_readable(&self) -> bool {
        // we never want to have a human-readable serialization
        false
    }

    #[inline(always)]
    fn deserialize_any<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }

    #[inline(always)]
    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_bool(self.pop_word().map(|v| v == 1)?)
    }

    #[inline(always)]
    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i8(self.pop_word()? as i8)
    }

    #[inline(always)]
    fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i16(self.pop_word()? as i16)
    }

    #[inline(always)]
    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i32(self.pop_word()? as i32)
    }

    #[inline(always)]
    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i64(self.pop_word()? as i64)
    }

    #[inline(always)]
    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u8(self.pop_word()? as u8)
    }

    #[inline(always)]
    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u16(self.pop_word()? as u16)
    }

    #[inline(always)]
    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u32(self.pop_word()? as u32)
    }

    #[inline(always)]
    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u64(self.pop_word()?)
    }

    #[inline(always)]
    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_f32(f32::from_bits(self.pop_word()? as u32))
    }

    #[inline(always)]
    fn deserialize_f64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_f64(f64::from_bits(self.pop_word()?))
    }

    #[inline(always)]
    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_char(
            char::from_u32(self.pop_word()? as u32).ok_or_else(|| Error::new(Code::InvArgs))?,
        )
    }

    #[inline(always)]
    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_borrowed_str(self.pop_str_slice()?)
    }

    #[inline(always)]
    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_string(self.pop_str()?)
    }

    #[inline(always)]
    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_borrowed_bytes(self.pop_byte_slice()?)
    }

    #[inline(always)]
    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_byte_buf(self.pop_byte_vec()?)
    }

    #[inline(always)]
    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        if self.pos >= self.slice.len() {
            return Err(Error::new(Code::InvArgs));
        }

        // only supported for primitive integers
        if self.slice[self.pos] == !0 {
            self.pos += 1;
            visitor.visit_none()
        }
        else {
            visitor.visit_some(self)
        }
    }

    #[inline(always)]
    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_unit()
    }

    #[inline(always)]
    fn deserialize_unit_struct<V>(
        self,
        _name: &'static str,
        _visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }

    #[inline(always)]
    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    #[inline(always)]
    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        let len = self.pop_word()? as usize;
        visitor.visit_seq(SizedSeqAccess {
            de: self,
            pos: 0,
            len,
        })
    }

    #[inline(always)]
    fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_seq(self)
    }

    #[inline(always)]
    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        _len: usize,
        _visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }

    #[inline(always)]
    fn deserialize_map<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }

    #[inline(always)]
    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_seq(self)
    }

    #[inline(always)]
    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_enum(self)
    }

    #[inline(always)]
    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_u32(visitor)
    }

    #[inline(always)]
    fn deserialize_ignored_any<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }
}

impl<'de, 'a> SeqAccess<'de> for &'a mut M3Deserializer<'de> {
    type Error = Error;

    #[inline(always)]
    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        seed.deserialize(&mut **self).map(Some)
    }
}

struct SizedSeqAccess<'de, 'a> {
    de: &'a mut M3Deserializer<'de>,
    pos: usize,
    len: usize,
}

impl<'de, 'a> SeqAccess<'de> for SizedSeqAccess<'de, 'a> {
    type Error = Error;

    #[inline(always)]
    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        if self.pos >= self.len {
            Ok(None)
        }
        else {
            self.pos += 1;
            seed.deserialize(&mut *self.de).map(Some)
        }
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.len)
    }
}

impl<'de, 'a> EnumAccess<'de> for &'a mut M3Deserializer<'de> {
    type Error = Error;
    type Variant = Self;

    #[inline(always)]
    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        let value = seed.deserialize(&mut *self)?;
        Ok((value, self))
    }
}

impl<'de, 'a> VariantAccess<'de> for &'a mut M3Deserializer<'de> {
    type Error = Error;

    #[inline(always)]
    fn unit_variant(self) -> Result<(), Self::Error> {
        Ok(())
    }

    #[inline(always)]
    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        seed.deserialize(self)
    }

    #[inline(always)]
    fn tuple_variant<V>(self, _len: usize, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }

    #[inline(always)]
    fn struct_variant<V>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_seq(self)
    }
}
