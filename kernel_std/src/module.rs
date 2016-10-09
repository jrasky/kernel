use collections::{String, Vec};

use uuid::Uuid;

use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug)]
pub enum Placement {
    Absolute(u64), // text depends on being loaded at an exact memory address
    Relative { // text depends on being loaded relative to another text
        to: Uuid,
        offset: u64
    },
    Arbitrary(u64) // text does not depend on being loaded at a certain address
    // except maybe some alignment requirements
}

#[derive(Debug)]
pub enum Data {
    Offset(u64), // Text follows after Module at this offset
    Direct(Vec<u8>), // Text data as a byte vector
    Empty // Explicitly empty text data
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Port {
    pub id: Uuid, // port type
    pub offset: u64, // offset into owning text
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Dependency {
    pub id: Uuid, // port type being depended on
    pub places: Vec<u64> // list of offsets to fill port address into
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Header {
    pub id: Uuid, // unique ID for this text
    pub base: Placement, // how to place this text in memory
    pub size: u64, // length of this text
    pub write: bool, // whether this text should be writeable
    pub execute: bool, // whether this text should be executable
    pub provides: Vec<Port>, // ports this text provides
    pub depends: Vec<Dependency>, // ports this text depends on 
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Text {
    pub id: Uuid, // unique ID for this text, should match the Header
    pub data: Data // the data for this text
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Module {
    pub magic: Uuid, // 0af979b7-02c3-4ca6-b354-b709bec81199
    pub id: Uuid, // unique ID for this module
    pub headers: Vec<Header>, // headers provided by this module
    pub texts: Vec<Text> // texts provided by this module
}


impl Serialize for Data {
    fn serialize<S: Serializer>(&self, serializer: &mut S) -> Result<(), S::Error> {
        match self {
            &Data::Offset(offset) => {
                serializer.serialize_newtype_variant("Data", 0, "Offset", offset)
            }
            &Data::Direct(ref bytes) => {
                serializer.serialize_newtype_variant("Data", 1, "Direct", bytes)
            }
            &Data::Empty => {
                serializer.serialize_unit_variant("Data", 2, "Empty")
            }
        }
    }
}

impl Serialize for Placement {
    fn serialize<S: Serializer>(&self, serializer: &mut S) -> Result<(), S::Error> {
        match self {
            &Placement::Absolute(address) => {
                serializer.serialize_newtype_variant("Placement", 0, "Absolute", address)
            }
            &Placement::Relative { ref to, offset }  => {
                let mut state = try!(serializer.serialize_struct_variant("Placement", 1, "Relative", 2));
                try!(serializer.serialize_struct_variant_elt(&mut state, "to", to));
                try!(serializer.serialize_struct_variant_elt(&mut state, "offset", offset));
                serializer.serialize_struct_variant_end(state)
            }
            &Placement::Arbitrary(align) => {
                serializer.serialize_newtype_variant("Placement", 2, "Arbitrary", align)
            }
        }
    }
}

impl Deserialize for Placement {
    fn deserialize<D: Deserializer>(deserializer: &mut D) -> Result<Self, D::Error> {
        enum Variant {
            Absolute,
            Relative,
            Arbitrary
        }

        impl de::Deserialize for Variant {
            fn deserialize<D: Deserializer>(deserializer: &mut D) -> Result<Self, D::Error> {
                struct Visitor;
                
                impl de::Visitor for Visitor {
                    type Value = Variant;

                    fn visit_usize<E: de::Error>(&mut self, value: usize) -> Result<Variant, E> {
                        match value {
                            0 => Ok(Variant::Absolute),
                            1 => Ok(Variant::Relative),
                            2 => Ok(Variant::Arbitrary),
                            _ => Err(de::Error::invalid_value("expected a variant"))
                        }
                    }

                    fn visit_str<E: de::Error>(&mut self, value: &str) -> Result<Variant, E> {
                        match value {
                            "Absolute" => Ok(Variant::Absolute),
                            "Relative" => Ok(Variant::Relative),
                            "Arbitrary" => Ok(Variant::Arbitrary),
                            _ => Err(de::Error::unknown_variant(value))
                        }
                    }

                    fn visit_bytes<E: de::Error>(&mut self, value: &[u8]) -> Result<Variant, E> {
                        match value {
                            b"Absolute" => Ok(Variant::Absolute),
                            b"Relative" => Ok(Variant::Relative),
                            b"Arbitrary" => Ok(Variant::Arbitrary),
                            _ => {
                                let value = String::from_utf8_lossy(value);
                                Err(de::Error::unknown_variant(&value))
                            }
                        }
                    }
                }

                deserializer.deserialize_struct_field(Visitor)
            }
        }

        struct Visitor;

        impl de::EnumVisitor for Visitor {
            type Value = Placement;

            fn visit<V: de::VariantVisitor>(&mut self, mut visitor: V) -> Result<Placement, V::Error> {
                match try!(visitor.visit_variant()) {
                    Variant::Absolute => {
                        Ok(Placement::Absolute(try!(visitor.visit_newtype())))
                    }
                    Variant::Relative => {
                        enum Field {
                            To,
                            Offset,
                            Ignore
                        }

                        impl de::Deserialize for Field {
                            fn deserialize<D: Deserializer>(deserializer: &mut D) -> Result<Self, D::Error> {
                                struct Visitor;
                                
                                impl de::Visitor for Visitor {
                                    type Value = Field;

                                    fn visit_usize<E: de::Error>(&mut self, value: usize) -> Result<Field, E> {
                                        match value {
                                            0 => Ok(Field::To),
                                            1 => Ok(Field::Offset),
                                            _ => Ok(Field::Ignore)
                                        }
                                    }

                                    fn visit_str<E: de::Error>(&mut self, value: &str) -> Result<Field, E> {
                                        match value {
                                            "to" => Ok(Field::To),
                                            "offset" => Ok(Field::Offset),
                                            _ => Ok(Field::Ignore)
                                        }
                                    }

                                    fn visit_bytes<E: de::Error>(&mut self, value: &[u8]) -> Result<Field, E> {
                                        match value {
                                            b"to" => Ok(Field::To),
                                            b"offset" => Ok(Field::Offset),
                                            _ => Ok(Field::Ignore)
                                        }
                                    }
                                }

                                deserializer.deserialize_struct_field(Visitor)
                            }
                        }

                        struct Visitor;

                        impl de::Visitor for Visitor {
                            type Value = Placement;

                            fn visit_seq<V: de::SeqVisitor>(&mut self, mut visitor: V) -> Result<Placement, V::Error> {
                                let to: Uuid = match try!(visitor.visit()) {
                                    Some(value) => value,
                                    None => {
                                        try!(visitor.end());
                                        return Err(de::Error::invalid_length(0));
                                    }
                                };

                                let offset: u64 = match try!(visitor.visit()) {
                                    Some(value) => value,
                                    None => {
                                        try!(visitor.end());
                                        return Err(de::Error::invalid_length(1));
                                    }
                                };

                                try!(visitor.end());

                                Ok(Placement::Relative {
                                    to: to,
                                    offset: offset
                                })
                            }

                            fn visit_map<V: de::MapVisitor>(&mut self, mut visitor: V) -> Result<Placement, V::Error> {
                                let mut to: Option<Uuid> = None;
                                let mut offset: Option<u64> = None;

                                while let Some(key) = try!(visitor.visit_key()) {
                                    match key {
                                        Field::To => {
                                            if to.is_some() {
                                                return Err(de::Error::duplicate_field("to"));
                                            }

                                            to = Some(try!(visitor.visit_value()));
                                        }
                                        Field::Offset => {
                                            if offset.is_some() {
                                                return Err(de::Error::duplicate_field("offset"));
                                            }

                                            offset = Some(try!(visitor.visit_value()));
                                        }
                                        Field::Ignore => {
                                            try!(visitor.visit_value::<de::impls::IgnoredAny>());
                                        }
                                    }
                                }

                                try!(visitor.end());

                                Ok(Placement::Relative {
                                    to: try!(to.map_or_else(|| visitor.missing_field("to"), |v| Ok(v))),
                                    offset: try!(offset.map_or_else(|| visitor.missing_field("offset"), |v| Ok(v)))
                                })
                            }
                        }

                        const FIELDS: &'static [&'static str] = &["to", "offset"];
                        visitor.visit_struct(FIELDS, Visitor)
                    }
                    Variant::Arbitrary => {
                        Ok(Placement::Arbitrary(try!(visitor.visit_newtype())))
                    }
                }
            }
        }

        const VARIANTS: &'static [&'static str] = &["Absolute", "Relative", "Arbitrary"];
        deserializer.deserialize_enum("Placement", VARIANTS, Visitor)
    }
}

impl Deserialize for Data {
    fn deserialize<D: Deserializer>(deserializer: &mut D) -> Result<Self, D::Error> {
        enum Variant {
            Offset,
            Direct,
            Empty
        }

        impl de::Deserialize for Variant {
            fn deserialize<D: Deserializer>(deserializer: &mut D) -> Result<Self, D::Error> {
                struct Visitor;
                
                impl de::Visitor for Visitor {
                    type Value = Variant;

                    fn visit_usize<E: de::Error>(&mut self, value: usize) -> Result<Variant, E> {
                        match value {
                            0 => Ok(Variant::Offset),
                            1 => Ok(Variant::Direct),
                            2 => Ok(Variant::Empty),
                            _ => Err(de::Error::invalid_value("expected a variant"))
                        }
                    }

                    fn visit_str<E: de::Error>(&mut self, value: &str) -> Result<Variant, E> {
                        match value {
                            "Offset" => Ok(Variant::Offset),
                            "Direct" => Ok(Variant::Direct),
                            "Empty" => Ok(Variant::Empty),
                            _ => Err(de::Error::unknown_variant(value))
                        }
                    }

                    fn visit_bytes<E: de::Error>(&mut self, value: &[u8]) -> Result<Variant, E> {
                        match value {
                            b"Offset" => Ok(Variant::Offset),
                            b"Direct" => Ok(Variant::Direct),
                            b"Empty" => Ok(Variant::Empty),
                            _ => {
                                let value = String::from_utf8_lossy(value);
                                Err(de::Error::unknown_variant(&value))
                            }
                        }
                    }
                }

                deserializer.deserialize_struct_field(Visitor)
            }
        }

        struct Visitor;

        impl de::EnumVisitor for Visitor {
            type Value = Data;

            fn visit<V: de::VariantVisitor>(&mut self, mut visitor: V) -> Result<Data, V::Error> {
                match try!(visitor.visit_variant()) {
                    Variant::Offset => {
                        Ok(Data::Offset(try!(visitor.visit_newtype())))
                    }
                    Variant::Direct => {
                        Ok(Data::Direct(try!(visitor.visit_newtype())))
                    }
                    Variant::Empty => {
                        try!(visitor.visit_unit());
                        Ok(Data::Empty)
                    }
                }
            }
        }

        const VARIANTS: &'static [&'static str] = &["Offset", "Direct", "Empty"];
        deserializer.deserialize_enum("Data", VARIANTS, Visitor)
    }
}
