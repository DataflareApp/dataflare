use query::Value;
use std::str::FromStr;

pub(super) fn decode_value(val: String, datatype: &str) -> Value {
    if val == "ᴺᵁᴸᴸ" && datatype.starts_with("Nullable(") {
        return Value::Null;
    }
    match datatype {
        "Bool" | "Nullable(Bool)" => decode(val, Value::Bool),
        "Int8" | "Nullable(Int8)" => decode(val, Value::I8),
        "UInt8" | "Nullable(UInt8)" => decode(val, Value::U8),
        "Int16" | "Nullable(Int16)" => decode(val, Value::I16),
        "UInt16" | "Nullable(UInt16)" => decode(val, Value::U16),
        "Int32" | "Nullable(Int32)" => decode(val, Value::I32),
        "UInt32" | "Nullable(UInt32)" => decode(val, Value::U32),
        "Int64" | "Nullable(Int64)" => decode(val, Value::I64),
        "UInt64" | "Nullable(UInt64)" => decode(val, Value::U64),
        "Float32" | "Nullable(Float32)" => decode(val, Value::F32),
        "Float64" | "Nullable(Float64)" => decode(val, Value::F64),
        _ => Value::String(val),
    }
}

#[inline]
fn decode<T: FromStr>(val: String, f: fn(T) -> Value) -> Value {
    match val.parse::<T>() {
        Ok(val) => f(val),
        Err(_) => Value::String(val),
    }
}

#[cfg(test)]
mod tests {
    use super::decode_value;
    use query::Value;

    #[test]
    fn decode_value_null_for_nullable_type() {
        assert_eq!(decode_value("ᴺᵁᴸᴸ".into(), "Nullable(String)"), Value::Null);
        assert_eq!(decode_value("ᴺᵁᴸᴸ".into(), "Nullable(Int32)"), Value::Null);
        assert_eq!(decode_value("ᴺᵁᴸᴸ".into(), "Nullable(Bool)"), Value::Null);
    }

    #[test]
    fn decode_value_bool() {
        assert_eq!(decode_value("true".into(), "Bool"), Value::Bool(true));
        assert_eq!(decode_value("false".into(), "Bool"), Value::Bool(false));
        assert_eq!(
            decode_value("true".into(), "Nullable(Bool)"),
            Value::Bool(true)
        );
    }

    #[test]
    fn decode_value_signed_integers() {
        assert_eq!(decode_value("127".into(), "Int8"), Value::I8(127));
        assert_eq!(decode_value("32767".into(), "Int16"), Value::I16(32767));
        assert_eq!(
            decode_value("2147483647".into(), "Int32"),
            Value::I32(2147483647)
        );
        assert_eq!(
            decode_value("9223372036854775807".into(), "Int64"),
            Value::I64(i64::MAX)
        );
    }

    #[test]
    fn decode_value_unsigned_integers() {
        assert_eq!(decode_value("255".into(), "UInt8"), Value::U8(255));
        assert_eq!(decode_value("65535".into(), "UInt16"), Value::U16(65535));
        assert_eq!(
            decode_value("4294967295".into(), "UInt32"),
            Value::U32(4294967295)
        );
        assert_eq!(
            decode_value("18446744073709551615".into(), "UInt64"),
            Value::U64(u64::MAX)
        );
    }

    #[test]
    fn decode_value_floats() {
        assert_eq!(decode_value("3.14".into(), "Float32"), Value::F32(3.14));
        assert_eq!(
            decode_value("3.141592653589793".into(), "Float64"),
            Value::F64(3.141592653589793)
        );
    }

    #[test]
    fn decode_value_nullable_numeric_types() {
        assert_eq!(decode_value("42".into(), "Nullable(Int32)"), Value::I32(42));
        assert_eq!(decode_value("-1".into(), "Nullable(Int8)"), Value::I8(-1));
        assert_eq!(
            decode_value("100".into(), "Nullable(UInt64)"),
            Value::U64(100)
        );
        assert_eq!(
            decode_value("2.5".into(), "Nullable(Float32)"),
            Value::F32(2.5)
        );
    }

    #[test]
    fn decode_value_unknown_type_returns_string() {
        assert_eq!(
            decode_value("2024-01-01".into(), "Date"),
            Value::String("2024-01-01".into())
        );
        assert_eq!(
            decode_value("hello".into(), "FixedString(10)"),
            Value::String("hello".into())
        );
        assert_eq!(
            decode_value("[1,2,3]".into(), "Array(UInt32)"),
            Value::String("[1,2,3]".into())
        );
    }

    #[test]
    fn decode_value_unparseable_falls_back_to_string() {
        assert_eq!(
            decode_value("not_a_number".into(), "Int32"),
            Value::String("not_a_number".into())
        );
        assert_eq!(
            decode_value("not_a_bool".into(), "Bool"),
            Value::String("not_a_bool".into())
        );
        assert_eq!(
            decode_value("256".into(), "UInt8"),
            Value::String("256".into())
        );
        assert_eq!(
            decode_value("-1".into(), "UInt32"),
            Value::String("-1".into())
        );
    }
}
