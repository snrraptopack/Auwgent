//! Runtime value representation.
//!
//! Every value that flows through a quew program at runtime is represented by
//! the [`Value`] enum. It mirrors the Quew type system:
//!
//! | Quew type | `Value` variant |
//! |-----------|-----------------|
//! | `string`  | `Value::String` |
//! | `number`  | `Value::Number` |
//! | `float`   | `Value::Float`  |
//! | `bool`    | `Value::Bool`   |
//! | `null`    | `Value::Null`   |
//! | `T[]`     | `Value::Array`  |
//! | record    | `Value::Object` |
//!
//! `Value` is intentionally simple — it carries data, not type metadata. The
//! compiler already proved type correctness; the runtime trusts the IR.

use std::fmt;

use indexmap::IndexMap;

/// A runtime value produced by executing a quew graph.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    String(String),
    Number(i64),
    Float(f64),
    Bool(bool),
    Null,
    Object(IndexMap<String, Value>),
    Array(Vec<Value>),
}

impl Value {
    /// Returns the name of the Quew type this value represents.
    ///
    /// Used for error messages and debugging.
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::String(_) => "string",
            Value::Number(_) => "number",
            Value::Float(_) => "float",
            Value::Bool(_) => "bool",
            Value::Null => "null",
            Value::Object(_) => "object",
            Value::Array(_) => "array",
        }
    }

    // ── Typed accessors ───────────────────────────────────────────────────────

    /// Extract the inner `String`, if this is a `Value::String`.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    /// Extract the inner `i64`, if this is a `Value::Number`.
    pub fn as_number(&self) -> Option<i64> {
        match self {
            Value::Number(n) => Some(*n),
            _ => None,
        }
    }

    /// Extract the inner `f64`, if this is a `Value::Float`.
    pub fn as_float(&self) -> Option<f64> {
        match self {
            Value::Float(f) => Some(*f),
            _ => None,
        }
    }

    /// Extract the inner `bool`, if this is a `Value::Bool`.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Extract a reference to the inner array, if this is a `Value::Array`.
    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }

    /// Extract a reference to the inner object, if this is a `Value::Object`.
    pub fn as_object(&self) -> Option<&IndexMap<String, Value>> {
        match self {
            Value::Object(o) => Some(o),
            _ => None,
        }
    }

    // ── Truthiness ────────────────────────────────────────────────────────────

    /// Returns whether this value is truthy in a boolean context.
    ///
    /// Quew truthiness rules:
    /// - `false` and `null` are falsy
    /// - everything else is truthy (including `0`, `0.0`, and `""`)
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Bool(false) | Value::Null => false,
            _ => true,
        }
    }

    // ── Binary operators ──────────────────────────────────────────────────────

    /// Add two values. Supported for `Number + Number`, `Float + Float`,
    /// `Number + Float`, `Float + Number`, and `String + String`.
    pub fn add(&self, other: &Value) -> Result<Value, ValueError> {
        match (self, other) {
            (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a + b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
            (Value::Number(a), Value::Float(b)) => Ok(Value::Float(*a as f64 + b)),
            (Value::Float(a), Value::Number(b)) => Ok(Value::Float(a + *b as f64)),
            (Value::String(a), Value::String(b)) => Ok(Value::String(format!("{a}{b}"))),
            _ => Err(ValueError::TypeMismatch {
                op: "add",
                left: self.type_name(),
                right: other.type_name(),
            }),
        }
    }

    /// Subtract two values. Supported for numeric types.
    pub fn sub(&self, other: &Value) -> Result<Value, ValueError> {
        match (self, other) {
            (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a - b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a - b)),
            (Value::Number(a), Value::Float(b)) => Ok(Value::Float(*a as f64 - b)),
            (Value::Float(a), Value::Number(b)) => Ok(Value::Float(a - *b as f64)),
            _ => Err(ValueError::TypeMismatch {
                op: "sub",
                left: self.type_name(),
                right: other.type_name(),
            }),
        }
    }

    /// Multiply two values. Supported for numeric types.
    pub fn mul(&self, other: &Value) -> Result<Value, ValueError> {
        match (self, other) {
            (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a * b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a * b)),
            (Value::Number(a), Value::Float(b)) => Ok(Value::Float(*a as f64 * b)),
            (Value::Float(a), Value::Number(b)) => Ok(Value::Float(a * *b as f64)),
            _ => Err(ValueError::TypeMismatch {
                op: "mul",
                left: self.type_name(),
                right: other.type_name(),
            }),
        }
    }

    /// Divide two values. Supported for numeric types.
    pub fn div(&self, other: &Value) -> Result<Value, ValueError> {
        match (self, other) {
            (Value::Number(a), Value::Number(b)) => {
                if *b == 0 {
                    return Err(ValueError::DivisionByZero);
                }
                Ok(Value::Number(a / b))
            }
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a / b)),
            (Value::Number(a), Value::Float(b)) => Ok(Value::Float(*a as f64 / b)),
            (Value::Float(a), Value::Number(b)) => Ok(Value::Float(a / *b as f64)),
            _ => Err(ValueError::TypeMismatch {
                op: "div",
                left: self.type_name(),
                right: other.type_name(),
            }),
        }
    }

    /// Remainder of two values. Supported for `Number % Number`.
    pub fn rem(&self, other: &Value) -> Result<Value, ValueError> {
        match (self, other) {
            (Value::Number(a), Value::Number(b)) => {
                if *b == 0 {
                    return Err(ValueError::DivisionByZero);
                }
                Ok(Value::Number(a % b))
            }
            _ => Err(ValueError::TypeMismatch {
                op: "rem",
                left: self.type_name(),
                right: other.type_name(),
            }),
        }
    }

    /// Equality comparison.
    pub fn eq_val(&self, other: &Value) -> Result<Value, ValueError> {
        // Different numeric types can be compared
        let comparable = match (self, other) {
            (Value::Number(a), Value::Number(b)) => Some(Value::Bool(a == b)),
            (Value::Float(a), Value::Float(b)) => Some(Value::Bool(a == b)),
            (Value::Number(a), Value::Float(b)) => Some(Value::Bool(*a as f64 == *b)),
            (Value::Float(a), Value::Number(b)) => Some(Value::Bool(*a == *b as f64)),
            _ => None,
        };

        if let Some(result) = comparable {
            return Ok(result);
        }

        // Same-type non-numeric comparison
        if std::mem::discriminant(self) == std::mem::discriminant(other) {
            Ok(Value::Bool(self == other))
        } else {
            Ok(Value::Bool(false))
        }
    }

    /// Inequality comparison.
    pub fn not_eq_val(&self, other: &Value) -> Result<Value, ValueError> {
        self.eq_val(other).map(|v| match v {
            Value::Bool(b) => Value::Bool(!b),
            _ => unreachable!(),
        })
    }

    /// Logical AND. Both operands must be bool.
    pub fn and(&self, other: &Value) -> Result<Value, ValueError> {
        match (self, other) {
            (Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(*a && *b)),
            _ => Err(ValueError::TypeMismatch {
                op: "and",
                left: self.type_name(),
                right: other.type_name(),
            }),
        }
    }

    /// Logical OR. Both operands must be bool.
    pub fn or(&self, other: &Value) -> Result<Value, ValueError> {
        match (self, other) {
            (Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(*a || *b)),
            _ => Err(ValueError::TypeMismatch {
                op: "or",
                left: self.type_name(),
                right: other.type_name(),
            }),
        }
    }

    /// Less-than comparison. Supported for numeric types.
    pub fn lt(&self, other: &Value) -> Result<Value, ValueError> {
        match (self, other) {
            (Value::Number(a), Value::Number(b)) => Ok(Value::Bool(a < b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a < b)),
            (Value::Number(a), Value::Float(b)) => Ok(Value::Bool((*a as f64) < *b)),
            (Value::Float(a), Value::Number(b)) => Ok(Value::Bool(*a < *b as f64)),
            _ => Err(ValueError::TypeMismatch {
                op: "lt",
                left: self.type_name(),
                right: other.type_name(),
            }),
        }
    }

    /// Less-than-or-equal comparison. Supported for numeric types.
    pub fn lte(&self, other: &Value) -> Result<Value, ValueError> {
        match (self, other) {
            (Value::Number(a), Value::Number(b)) => Ok(Value::Bool(a <= b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a <= b)),
            (Value::Number(a), Value::Float(b)) => Ok(Value::Bool((*a as f64) <= *b)),
            (Value::Float(a), Value::Number(b)) => Ok(Value::Bool(*a <= *b as f64)),
            _ => Err(ValueError::TypeMismatch {
                op: "lte",
                left: self.type_name(),
                right: other.type_name(),
            }),
        }
    }

    /// Greater-than comparison. Supported for numeric types.
    pub fn gt(&self, other: &Value) -> Result<Value, ValueError> {
        match (self, other) {
            (Value::Number(a), Value::Number(b)) => Ok(Value::Bool(a > b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a > b)),
            (Value::Number(a), Value::Float(b)) => Ok(Value::Bool((*a as f64) > *b)),
            (Value::Float(a), Value::Number(b)) => Ok(Value::Bool(*a > *b as f64)),
            _ => Err(ValueError::TypeMismatch {
                op: "gt",
                left: self.type_name(),
                right: other.type_name(),
            }),
        }
    }

    /// Greater-than-or-equal comparison. Supported for numeric types.
    pub fn gte(&self, other: &Value) -> Result<Value, ValueError> {
        match (self, other) {
            (Value::Number(a), Value::Number(b)) => Ok(Value::Bool(a >= b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a >= b)),
            (Value::Number(a), Value::Float(b)) => Ok(Value::Bool((*a as f64) >= *b)),
            (Value::Float(a), Value::Number(b)) => Ok(Value::Bool(*a >= *b as f64)),
            _ => Err(ValueError::TypeMismatch {
                op: "gte",
                left: self.type_name(),
                right: other.type_name(),
            }),
        }
    }

    /// Unary negation (`-`). Supported for numeric types.
    pub fn neg(&self) -> Result<Value, ValueError> {
        match self {
            Value::Number(n) => Ok(Value::Number(-n)),
            Value::Float(f) => Ok(Value::Float(-f)),
            _ => Err(ValueError::UnaryTypeMismatch {
                op: "neg",
                ty: self.type_name(),
            }),
        }
    }

    /// Unary logical NOT (`not`).
    pub fn not(&self) -> Result<Value, ValueError> {
        match self {
            Value::Bool(b) => Ok(Value::Bool(!b)),
            _ => Err(ValueError::UnaryTypeMismatch {
                op: "not",
                ty: self.type_name(),
            }),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::String(s) => write!(f, "{s}"),
            Value::Number(n) => write!(f, "{n}"),
            Value::Float(fl) => write!(f, "{fl}"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Null => write!(f, "null"),
            Value::Object(o) => {
                let fields: Vec<String> = o.iter().map(|(k, v)| format!("{k}: {v}")).collect();
                write!(f, "{{ {}}}", fields.join(", "))
            }
            Value::Array(a) => {
                let items: Vec<String> = a.iter().map(|v| format!("{v}")).collect();
                write!(f, "[{}]", items.join(", "))
            }
        }
    }
}

/// An error produced by invalid value operations.
#[derive(Debug, Clone, PartialEq)]
pub enum ValueError {
    TypeMismatch {
        op: &'static str,
        left: &'static str,
        right: &'static str,
    },
    UnaryTypeMismatch {
        op: &'static str,
        ty: &'static str,
    },
    DivisionByZero,
}

impl fmt::Display for ValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValueError::TypeMismatch { op, left, right } => {
                write!(f, "cannot apply '{op}' to {left} and {right}")
            }
            ValueError::UnaryTypeMismatch { op, ty } => {
                write!(f, "cannot apply '{op}' to {ty}")
            }
            ValueError::DivisionByZero => write!(f, "division by zero"),
        }
    }
}

impl std::error::Error for ValueError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_addition() {
        let a = Value::Number(3);
        let b = Value::Number(5);
        assert_eq!(a.add(&b).unwrap(), Value::Number(8));
    }

    #[test]
    fn float_addition_with_number() {
        let a = Value::Number(3);
        let b = Value::Float(2.5);
        let result = a.add(&b).unwrap();
        assert!(matches!(result, Value::Float(v) if (v - 5.5).abs() < f64::EPSILON));
    }

    #[test]
    fn string_concatenation() {
        let a = Value::String("hello".into());
        let b = Value::String(" world".into());
        assert_eq!(a.add(&b).unwrap(), Value::String("hello world".into()));
    }

    #[test]
    fn mixed_type_addition_errors() {
        let a = Value::Number(3);
        let b = Value::String("x".into());
        assert!(a.add(&b).is_err());
    }

    #[test]
    fn division_by_zero() {
        let a = Value::Number(10);
        let b = Value::Number(0);
        assert_eq!(a.div(&b).unwrap_err(), ValueError::DivisionByZero);
    }

    #[test]
    fn number_comparison_across_types() {
        let a = Value::Number(5);
        let b = Value::Float(5.0);
        assert_eq!(a.eq_val(&b).unwrap(), Value::Bool(true));
    }

    #[test]
    fn different_types_are_not_equal() {
        let a = Value::Number(5);
        let b = Value::String("5".into());
        assert_eq!(a.eq_val(&b).unwrap(), Value::Bool(false));
    }

    #[test]
    fn logical_and_requires_bool() {
        let a = Value::Bool(true);
        let b = Value::Bool(false);
        assert_eq!(a.and(&b).unwrap(), Value::Bool(false));

        let c = Value::Number(1);
        assert!(a.and(&c).is_err());
    }

    #[test]
    fn truthiness() {
        assert!(Value::String("".into()).is_truthy()); // empty string is truthy
        assert!(Value::Number(0).is_truthy()); // zero is truthy
        assert!(Value::Float(0.0).is_truthy()); // zero float is truthy
        assert!(!Value::Bool(false).is_truthy());
        assert!(!Value::Null.is_truthy());
        assert!(Value::Bool(true).is_truthy());
    }

    #[test]
    fn accessors() {
        assert_eq!(Value::String("hi".into()).as_str(), Some("hi"));
        assert_eq!(Value::Number(42).as_number(), Some(42));
        assert_eq!(Value::Float(3.14).as_float(), Some(3.14));
        assert_eq!(Value::Bool(true).as_bool(), Some(true));
        assert_eq!(Value::Null.as_str(), None);
    }

    #[test]
    fn display_formatting() {
        assert_eq!(format!("{}", Value::Number(42)), "42");
        assert_eq!(format!("{}", Value::String("hi".into())), "hi");
        assert_eq!(format!("{}", Value::Null), "null");
    }
}
