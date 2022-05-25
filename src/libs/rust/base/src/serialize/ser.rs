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

use crate::errors::{Code, Error};
use crate::mem;
use crate::serialize::copy_from_str;
use serde::{ser, Serialize, Serializer};

// The serializer for serializing values into the slice
pub struct M3Serializer<'s> {
    slice: &'s mut [u64],
    pos: usize,
}

impl<'s> M3Serializer<'s> {
    #[inline(always)]
    pub fn new(slice: &'s mut [u64]) -> Self {
        M3Serializer { slice, pos: 0 }
    }

    #[inline(always)]
    pub fn size(&self) -> usize {
        self.pos * mem::size_of::<u64>()
    }

    #[inline(always)]
    pub fn words(&self) -> &[u64] {
        &self.slice[0..self.pos]
    }

    // serializes a given value into the slice
    #[inline(always)]
    pub fn push<T: Serialize>(&mut self, item: T) {
        item.serialize(self).unwrap();
    }

    #[inline(always)]
    pub fn push_word(&mut self, word: u64) {
        self.slice[self.pos] = word;
        self.pos += 1;
    }
}

impl<'s, 'a> Serializer for &'a mut M3Serializer<'s> {
    type Error = Error;
    type Ok = ();
    type SerializeMap = Self;
    type SerializeSeq = Self;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;

    #[inline(always)]
    fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
        self.push_word(v as u64);
        Ok(())
    }

    #[inline(always)]
    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
        self.push_word(v as u64);
        Ok(())
    }

    #[inline(always)]
    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        self.push_word(v as u64);
        Ok(())
    }

    #[inline(always)]
    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        self.push_word(v as u64);
        Ok(())
    }

    #[inline(always)]
    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        self.push_word(v as u64);
        Ok(())
    }

    #[inline(always)]
    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        self.push_word(v as u64);
        Ok(())
    }

    #[inline(always)]
    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        self.push_word(v as u64);
        Ok(())
    }

    #[inline(always)]
    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        self.push_word(v as u64);
        Ok(())
    }

    #[inline(always)]
    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        self.push_word(v as u64);
        Ok(())
    }

    #[inline(always)]
    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
        self.push_word(v as u64);
        Ok(())
    }

    #[inline(always)]
    fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
        self.push_word(v as u64);
        Ok(())
    }

    #[inline(always)]
    fn serialize_char(self, _v: char) -> Result<Self::Ok, Self::Error> {
        unimplemented!()
    }

    #[inline(always)]
    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        let len = v.len() + 1;
        self.push_word(len as u64);

        unsafe { copy_from_str(&mut self.slice[self.pos..], v) }

        self.pos += (len + 7) / 8;
        Ok(())
    }

    #[inline(always)]
    fn serialize_bytes(self, _v: &[u8]) -> Result<Self::Ok, Self::Error> {
        unimplemented!()
    }

    #[inline(always)]
    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        // only supported for primitive integers
        self.push_word(!0);
        Ok(())
    }

    #[inline(always)]
    fn serialize_some<T: ?Sized>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: serde::Serialize,
    {
        // only supported for primitive integers
        value.serialize(self)
    }

    #[inline(always)]
    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        unimplemented!()
    }

    #[inline(always)]
    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        unimplemented!()
    }

    #[inline(always)]
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        idx: u32,
        _variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.serialize_u32(idx)
    }

    #[inline(always)]
    fn serialize_newtype_struct<T: ?Sized>(
        self,
        _name: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: serde::Serialize,
    {
        unimplemented!()
    }

    #[inline(always)]
    fn serialize_newtype_variant<T: ?Sized>(
        self,
        _name: &'static str,
        idx: u32,
        _variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: serde::Serialize,
    {
        self.serialize_u32(idx)?;
        value.serialize(self)
    }

    #[inline(always)]
    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        match len {
            None => return Err(Error::new(Code::NotSup)),
            Some(l) => self.serialize_u64(l as u64)?,
        };
        Ok(self)
    }

    #[inline(always)]
    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Ok(self)
    }

    #[inline(always)]
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        unimplemented!()
    }

    #[inline(always)]
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        unimplemented!()
    }

    #[inline(always)]
    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        unimplemented!()
    }

    #[inline(always)]
    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Ok(self)
    }

    #[inline(always)]
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        idx: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        self.serialize_u32(idx)?;
        Ok(self)
    }
}

impl<'s, 'a> ser::SerializeSeq for &'a mut M3Serializer<'s> {
    type Error = Error;
    type Ok = ();

    #[inline(always)]
    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize,
    {
        value.serialize(&mut **self)
    }

    #[inline(always)]
    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

impl<'s, 'a> ser::SerializeTuple for &'a mut M3Serializer<'s> {
    type Error = Error;
    type Ok = ();

    #[inline(always)]
    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize,
    {
        value.serialize(&mut **self)
    }

    #[inline(always)]
    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

impl<'s, 'a> ser::SerializeTupleStruct for &'a mut M3Serializer<'s> {
    type Error = Error;
    type Ok = ();

    #[inline(always)]
    fn serialize_field<T: ?Sized>(&mut self, _value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize,
    {
        unimplemented!()
    }

    #[inline(always)]
    fn end(self) -> Result<Self::Ok, Self::Error> {
        unimplemented!()
    }
}

impl<'s, 'a> ser::SerializeTupleVariant for &'a mut M3Serializer<'s> {
    type Error = Error;
    type Ok = ();

    #[inline(always)]
    fn serialize_field<T: ?Sized>(&mut self, _value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize,
    {
        unimplemented!()
    }

    #[inline(always)]
    fn end(self) -> Result<Self::Ok, Self::Error> {
        unimplemented!()
    }
}

impl<'s, 'a> ser::SerializeMap for &'a mut M3Serializer<'s> {
    type Error = Error;
    type Ok = ();

    #[inline(always)]
    fn serialize_key<T: ?Sized>(&mut self, _key: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize,
    {
        unimplemented!()
    }

    #[inline(always)]
    fn serialize_value<T: ?Sized>(&mut self, _value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize,
    {
        unimplemented!()
    }

    #[inline(always)]
    fn end(self) -> Result<Self::Ok, Self::Error> {
        unimplemented!()
    }
}

impl<'s, 'a> ser::SerializeStruct for &'a mut M3Serializer<'s> {
    type Error = Error;
    type Ok = ();

    #[inline(always)]
    fn serialize_field<T: ?Sized>(
        &mut self,
        _key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error>
    where
        T: serde::Serialize,
    {
        value.serialize(&mut **self)
    }

    #[inline(always)]
    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

impl<'s, 'a> ser::SerializeStructVariant for &'a mut M3Serializer<'s> {
    type Error = Error;
    type Ok = ();

    #[inline(always)]
    fn serialize_field<T: ?Sized>(
        &mut self,
        _key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error>
    where
        T: serde::Serialize,
    {
        value.serialize(&mut **self)
    }

    #[inline(always)]
    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}
