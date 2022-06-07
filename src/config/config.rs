use std::collections::HashMap;
use std::string;

pub type Array = Vec<ConfigValue>;

pub type Hash = HashMap<String, ConfigValue>;

#[derive(Debug, Clone, PartialEq)]
pub enum ConfigValue {
    Float(f32),
    Integer(i32),
    String(string::String),
    Boolean(bool),
    Array(self::Array),
    Hash(self::Hash),
    Null,
    Invalid,
}

macro_rules! define_as (
    ($name:ident, $t:ident, $yt:ident) => (
pub fn $name(&self) -> Option<$t> {
    match *self {
        ConfigValue::$yt(v) => Some(v),
        _ => None
    }
}
    );
);

macro_rules! define_as_ref (
    ($name:ident, $t:ty, $yt:ident) => (
pub fn $name(&self) -> Option<$t> {
    match *self {
        ConfigValue::$yt(ref v) => Some(v),
        _ => None
    }
}
    );
);

impl ConfigValue {
    define_as!(as_bool, bool, Boolean);
    define_as!(as_i32, i32, Integer);
    define_as!(as_f32, f32, Float);

    define_as_ref!(as_str, &str, String);
    define_as_ref!(as_hash, &Hash, Hash);
    define_as_ref!(as_vec, &Array, Array);

    pub fn is_null(&self) -> bool {
        match *self {
            ConfigValue::Null => true,
            _ => false,
        }
    }

    pub fn is_invalid(&self) -> bool {
        match *self {
            ConfigValue::Invalid => true,
            _ => false,
        }
    }

    pub fn is_array(&self) -> bool {
        match *self {
            ConfigValue::Array(_) => true,
            _ => false,
        }
    }

    pub fn is_hash(&self) -> bool {
        match *self {
            ConfigValue::Hash(_) => true,
            _ => false,
        }
    }
}
