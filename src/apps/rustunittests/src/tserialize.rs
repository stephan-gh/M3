/*
 * Copyright (C) 2024 Nils Asmussen, Barkhausen Institut
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

use m3::col::{String, Vec};
use m3::serde::{Deserialize, Serialize};
use m3::serialize::{M3Deserializer, M3Serializer, VecSink};
use m3::test::WvTester;
use m3::{vec, wv_assert_eq, wv_run_test};

pub fn run(t: &mut dyn WvTester) {
    wv_run_test!(t, basics);
    wv_run_test!(t, strings);
    wv_run_test!(t, sequences);
    wv_run_test!(t, structs);
}

fn basics(t: &mut dyn WvTester) {
    let mut vec = vec![];
    let mut ser = M3Serializer::new(VecSink::new(&mut vec));
    ser.push(1u8);
    ser.push(2i8);
    ser.push(3u16);
    ser.push(4i16);
    ser.push(5u32);
    ser.push(6i32);
    ser.push(7u64);
    ser.push(8i64);
    ser.push(9.5f32);
    ser.push(10.8f64);
    ser.push('a');
    ser.push(true);
    ser.push(());

    let mut de = M3Deserializer::new(&vec);
    wv_assert_eq!(t, de.pop::<u8>(), Ok(1u8));
    wv_assert_eq!(t, de.pop::<i8>(), Ok(2i8));
    wv_assert_eq!(t, de.pop::<u16>(), Ok(3u16));
    wv_assert_eq!(t, de.pop::<i16>(), Ok(4i16));
    wv_assert_eq!(t, de.pop::<u32>(), Ok(5u32));
    wv_assert_eq!(t, de.pop::<i32>(), Ok(6i32));
    wv_assert_eq!(t, de.pop::<u64>(), Ok(7u64));
    wv_assert_eq!(t, de.pop::<i64>(), Ok(8i64));
    wv_assert_eq!(t, de.pop::<f32>(), Ok(9.5f32));
    wv_assert_eq!(t, de.pop::<f64>(), Ok(10.8f64));
    wv_assert_eq!(t, de.pop::<char>(), Ok('a'));
    wv_assert_eq!(t, de.pop::<bool>(), Ok(true));
    wv_assert_eq!(t, de.pop::<()>(), Ok(()));
}

fn strings(t: &mut dyn WvTester) {
    let mut vec = vec![];
    let mut ser = M3Serializer::new(VecSink::new(&mut vec));
    ser.push("foo");
    ser.push(String::from("bar"));

    let mut de = M3Deserializer::new(&vec);
    wv_assert_eq!(t, de.pop::<&str>(), Ok("foo"));
    wv_assert_eq!(t, de.pop::<String>(), Ok(String::from("bar")));
}

fn sequences(t: &mut dyn WvTester) {
    let mut vec = vec![];
    let mut ser = M3Serializer::new(VecSink::new(&mut vec));
    ser.push((1, 2, 3));
    ser.push(vec![4, 5, 6]);

    let mut de = M3Deserializer::new(&vec);
    wv_assert_eq!(t, de.pop::<(_, _, _)>(), Ok((1, 2, 3)));
    wv_assert_eq!(t, de.pop::<Vec<_>>(), Ok(vec![4, 5, 6]));
}

fn structs(t: &mut dyn WvTester) {
    #[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
    #[serde(crate = "m3::serde")]
    struct Foo {
        a: u32,
        b: bool,
        c: String,
    }

    #[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
    #[serde(crate = "m3::serde")]
    struct FooUnit;

    #[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
    #[serde(crate = "m3::serde")]
    struct FooTupleStruct(u32, bool, u8);

    #[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
    #[serde(crate = "m3::serde")]
    enum Bar {
        A,
        B,
    }

    #[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
    #[serde(crate = "m3::serde")]
    enum Zoo {
        A(u32),
        B(bool),
    }

    #[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
    #[serde(crate = "m3::serde")]
    enum ZooTupleVariant {
        A(u32, u64),
        B(bool, u8),
    }

    #[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
    #[serde(crate = "m3::serde")]
    enum Zar {
        A { a: u8, b: usize },
        B { c: String },
    }

    let mut vec = vec![];
    let mut ser = M3Serializer::new(VecSink::new(&mut vec));
    ser.push(Foo {
        a: 1,
        b: true,
        c: String::from("test"),
    });
    ser.push(FooUnit);
    ser.push(FooTupleStruct(4, true, 16));
    ser.push(Bar::A);
    ser.push(Bar::B);
    ser.push(Zoo::A(2));
    ser.push(Zoo::B(false));
    ser.push(ZooTupleVariant::A(0, 10));
    ser.push(ZooTupleVariant::B(true, 255));
    ser.push(Zar::A { a: 4, b: 6 });
    ser.push(Zar::B {
        c: String::from("zar"),
    });

    let mut de = M3Deserializer::new(&vec);
    wv_assert_eq!(
        t,
        de.pop::<Foo>(),
        Ok(Foo {
            a: 1,
            b: true,
            c: String::from("test")
        })
    );
    wv_assert_eq!(t, de.pop::<FooUnit>(), Ok(FooUnit));
    wv_assert_eq!(
        t,
        de.pop::<FooTupleStruct>(),
        Ok(FooTupleStruct(4, true, 16))
    );
    wv_assert_eq!(t, de.pop::<Bar>(), Ok(Bar::A));
    wv_assert_eq!(t, de.pop::<Bar>(), Ok(Bar::B));
    wv_assert_eq!(t, de.pop::<Zoo>(), Ok(Zoo::A(2)));
    wv_assert_eq!(t, de.pop::<Zoo>(), Ok(Zoo::B(false)));
    wv_assert_eq!(
        t,
        de.pop::<ZooTupleVariant>(),
        Ok(ZooTupleVariant::A(0, 10))
    );
    wv_assert_eq!(
        t,
        de.pop::<ZooTupleVariant>(),
        Ok(ZooTupleVariant::B(true, 255))
    );
    wv_assert_eq!(t, de.pop::<Zar>(), Ok(Zar::A { a: 4, b: 6 }));
    wv_assert_eq!(
        t,
        de.pop::<Zar>(),
        Ok(Zar::B {
            c: String::from("zar")
        })
    );
}
