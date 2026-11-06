// Copyright (c) 2026, Radu Racariu.

//! Integers beyond what a JavaScript number holds exactly.
//!
//! serde-wasm-bindgen's JSON-compatible serializer rejects an `i64` or
//! `u64` outside `±(2^53 − 1)`. Its only alternative is a `BigInt`,
//! which is not JSON-compatible. libhaystack serializes a whole Number
//! that fits an `i64` as one, so a value such as `1e16` would fail
//! every method and drop every notification that carries it.
//! [`JsNumbers`] hands such an integer on as a float instead, the
//! number `JSON.parse` reads from its JSON text. That is exact for a
//! Haystack Number, which is a float to begin with.

use serde::ser::{self, Serialize, Serializer};

/// The largest integer a JavaScript number holds exactly, `2^53 − 1`
/// (`Number.MAX_SAFE_INTEGER`).
const MAX_SAFE_INTEGER: u64 = (1 << 53) - 1;

/// Serializes the value it wraps, and every value nested in it, with
/// each `i64` and `u64` outside `±(2^53 − 1)` as an `f64`. Every other
/// value is serialized as it would be unwrapped.
pub(crate) struct JsNumbers<'a, T: ?Sized>(pub(crate) &'a T);

impl<T: Serialize + ?Sized> Serialize for JsNumbers<'_, T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(JsNumberSerializer(serializer))
    }
}

/// The serializer [`JsNumbers`] serializes through: it converts the
/// unsafe integers and forwards everything else.
struct JsNumberSerializer<S>(S);

/// A compound serializer of `S`, whose elements, fields, keys and
/// values are serialized through [`JsNumbers`] too.
struct Compound<C>(C);

impl<S: Serializer> Serializer for JsNumberSerializer<S> {
    type Ok = S::Ok;
    type Error = S::Error;
    type SerializeSeq = Compound<S::SerializeSeq>;
    type SerializeTuple = Compound<S::SerializeTuple>;
    type SerializeTupleStruct = Compound<S::SerializeTupleStruct>;
    type SerializeTupleVariant = Compound<S::SerializeTupleVariant>;
    type SerializeMap = Compound<S::SerializeMap>;
    type SerializeStruct = Compound<S::SerializeStruct>;
    type SerializeStructVariant = Compound<S::SerializeStructVariant>;

    fn serialize_i64(self, v: i64) -> Result<S::Ok, S::Error> {
        if v.unsigned_abs() <= MAX_SAFE_INTEGER {
            self.0.serialize_i64(v)
        } else {
            self.0.serialize_f64(v as f64)
        }
    }

    fn serialize_u64(self, v: u64) -> Result<S::Ok, S::Error> {
        if v <= MAX_SAFE_INTEGER {
            self.0.serialize_u64(v)
        } else {
            self.0.serialize_f64(v as f64)
        }
    }

    fn serialize_bool(self, v: bool) -> Result<S::Ok, S::Error> {
        self.0.serialize_bool(v)
    }

    fn serialize_i8(self, v: i8) -> Result<S::Ok, S::Error> {
        self.0.serialize_i8(v)
    }

    fn serialize_i16(self, v: i16) -> Result<S::Ok, S::Error> {
        self.0.serialize_i16(v)
    }

    fn serialize_i32(self, v: i32) -> Result<S::Ok, S::Error> {
        self.0.serialize_i32(v)
    }

    fn serialize_i128(self, v: i128) -> Result<S::Ok, S::Error> {
        self.0.serialize_i128(v)
    }

    fn serialize_u8(self, v: u8) -> Result<S::Ok, S::Error> {
        self.0.serialize_u8(v)
    }

    fn serialize_u16(self, v: u16) -> Result<S::Ok, S::Error> {
        self.0.serialize_u16(v)
    }

    fn serialize_u32(self, v: u32) -> Result<S::Ok, S::Error> {
        self.0.serialize_u32(v)
    }

    fn serialize_u128(self, v: u128) -> Result<S::Ok, S::Error> {
        self.0.serialize_u128(v)
    }

    fn serialize_f32(self, v: f32) -> Result<S::Ok, S::Error> {
        self.0.serialize_f32(v)
    }

    fn serialize_f64(self, v: f64) -> Result<S::Ok, S::Error> {
        self.0.serialize_f64(v)
    }

    fn serialize_char(self, v: char) -> Result<S::Ok, S::Error> {
        self.0.serialize_char(v)
    }

    fn serialize_str(self, v: &str) -> Result<S::Ok, S::Error> {
        self.0.serialize_str(v)
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<S::Ok, S::Error> {
        self.0.serialize_bytes(v)
    }

    fn serialize_none(self) -> Result<S::Ok, S::Error> {
        self.0.serialize_none()
    }

    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<S::Ok, S::Error> {
        self.0.serialize_some(&JsNumbers(value))
    }

    fn serialize_unit(self) -> Result<S::Ok, S::Error> {
        self.0.serialize_unit()
    }

    fn serialize_unit_struct(self, name: &'static str) -> Result<S::Ok, S::Error> {
        self.0.serialize_unit_struct(name)
    }

    fn serialize_unit_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
    ) -> Result<S::Ok, S::Error> {
        self.0.serialize_unit_variant(name, variant_index, variant)
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        name: &'static str,
        value: &T,
    ) -> Result<S::Ok, S::Error> {
        self.0.serialize_newtype_struct(name, &JsNumbers(value))
    }

    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<S::Ok, S::Error> {
        self.0
            .serialize_newtype_variant(name, variant_index, variant, &JsNumbers(value))
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, S::Error> {
        self.0.serialize_seq(len).map(Compound)
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, S::Error> {
        self.0.serialize_tuple(len).map(Compound)
    }

    fn serialize_tuple_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, S::Error> {
        self.0.serialize_tuple_struct(name, len).map(Compound)
    }

    fn serialize_tuple_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, S::Error> {
        self.0
            .serialize_tuple_variant(name, variant_index, variant, len)
            .map(Compound)
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, S::Error> {
        self.0.serialize_map(len).map(Compound)
    }

    fn serialize_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, S::Error> {
        self.0.serialize_struct(name, len).map(Compound)
    }

    fn serialize_struct_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, S::Error> {
        self.0
            .serialize_struct_variant(name, variant_index, variant, len)
            .map(Compound)
    }

    fn is_human_readable(&self) -> bool {
        self.0.is_human_readable()
    }
}

impl<C: ser::SerializeSeq> ser::SerializeSeq for Compound<C> {
    type Ok = C::Ok;
    type Error = C::Error;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), C::Error> {
        self.0.serialize_element(&JsNumbers(value))
    }

    fn end(self) -> Result<C::Ok, C::Error> {
        self.0.end()
    }
}

impl<C: ser::SerializeTuple> ser::SerializeTuple for Compound<C> {
    type Ok = C::Ok;
    type Error = C::Error;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), C::Error> {
        self.0.serialize_element(&JsNumbers(value))
    }

    fn end(self) -> Result<C::Ok, C::Error> {
        self.0.end()
    }
}

impl<C: ser::SerializeTupleStruct> ser::SerializeTupleStruct for Compound<C> {
    type Ok = C::Ok;
    type Error = C::Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), C::Error> {
        self.0.serialize_field(&JsNumbers(value))
    }

    fn end(self) -> Result<C::Ok, C::Error> {
        self.0.end()
    }
}

impl<C: ser::SerializeTupleVariant> ser::SerializeTupleVariant for Compound<C> {
    type Ok = C::Ok;
    type Error = C::Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), C::Error> {
        self.0.serialize_field(&JsNumbers(value))
    }

    fn end(self) -> Result<C::Ok, C::Error> {
        self.0.end()
    }
}

impl<C: ser::SerializeMap> ser::SerializeMap for Compound<C> {
    type Ok = C::Ok;
    type Error = C::Error;

    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<(), C::Error> {
        self.0.serialize_key(&JsNumbers(key))
    }

    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), C::Error> {
        self.0.serialize_value(&JsNumbers(value))
    }

    fn serialize_entry<K: Serialize + ?Sized, V: Serialize + ?Sized>(
        &mut self,
        key: &K,
        value: &V,
    ) -> Result<(), C::Error> {
        self.0.serialize_entry(&JsNumbers(key), &JsNumbers(value))
    }

    fn end(self) -> Result<C::Ok, C::Error> {
        self.0.end()
    }
}

impl<C: ser::SerializeStruct> ser::SerializeStruct for Compound<C> {
    type Ok = C::Ok;
    type Error = C::Error;

    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), C::Error> {
        self.0.serialize_field(key, &JsNumbers(value))
    }

    fn skip_field(&mut self, key: &'static str) -> Result<(), C::Error> {
        self.0.skip_field(key)
    }

    fn end(self) -> Result<C::Ok, C::Error> {
        self.0.end()
    }
}

impl<C: ser::SerializeStructVariant> ser::SerializeStructVariant for Compound<C> {
    type Ok = C::Ok;
    type Error = C::Error;

    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), C::Error> {
        self.0.serialize_field(key, &JsNumbers(value))
    }

    fn skip_field(&mut self, key: &'static str) -> Result<(), C::Error> {
        self.0.skip_field(key)
    }

    fn end(self) -> Result<C::Ok, C::Error> {
        self.0.end()
    }
}
