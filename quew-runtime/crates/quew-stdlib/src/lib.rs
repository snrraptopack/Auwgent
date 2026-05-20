//! Quew standard library.
//!
//! Native builtin implementations registered via `#[quew_builtin]`.
//! Each annotated function automatically populates `NativeRegistry` at
//! link time through the `inventory` crate.

pub mod array;
pub mod string;
pub mod number;

#[cfg(test)]
mod tests {
    use quew_runtime::native::NativeRegistry;

    #[test]
    fn inventory_collects_string_builtins() {
        let reg = NativeRegistry::collect();
        assert!(reg.contains("std.string.len"), "std.string.len should be collected");
        assert!(reg.contains("std.string.is_empty"), "std.string.is_empty should be collected");
        assert!(reg.contains("std.string.contains"), "std.string.contains should be collected");
        assert!(reg.contains("std.string.starts_with"), "std.string.starts_with should be collected");
    }

    #[test]
    fn inventory_collects_number_builtins() {
        let reg = NativeRegistry::collect();
        assert!(reg.contains("std.number.abs"), "std.number.abs should be collected");
        assert!(reg.contains("std.number.clamp"), "std.number.clamp should be collected");
    }

    #[test]
    fn inventory_collects_array_builtins() {
        let reg = NativeRegistry::collect();
        assert!(reg.contains("std.array.len"), "std.array.len should be collected");
        assert!(reg.contains("std.array.get"), "std.array.get should be collected");
        assert!(reg.contains("std.array.push"), "std.array.push should be collected");
        assert!(reg.contains("std.array.pop"), "std.array.pop should be collected");
    }

    #[test]
    fn array_len_dispatch() {
        let reg = NativeRegistry::collect();
        let entry = reg.get("std.array.len").unwrap();
        let result = match &entry.handler {
            quew_runtime::native::NativeHandler::Sync(f) => {
                f(&[quew_runtime::value::Value::Array(vec![
                    quew_runtime::value::Value::Number(1),
                    quew_runtime::value::Value::Number(2),
                    quew_runtime::value::Value::Number(3),
                ])])
                .unwrap()
            }
        };
        assert_eq!(result, quew_runtime::value::Value::Number(3));
    }

    #[test]
    fn array_get_dispatch() {
        let reg = NativeRegistry::collect();
        let entry = reg.get("std.array.get").unwrap();
        let arr = quew_runtime::value::Value::Array(vec![
            quew_runtime::value::Value::String("a".into()),
            quew_runtime::value::Value::String("b".into()),
        ]);
        let result = match &entry.handler {
            quew_runtime::native::NativeHandler::Sync(f) => {
                f(&[arr, quew_runtime::value::Value::Number(1)]).unwrap()
            }
        };
        assert_eq!(result, quew_runtime::value::Value::String("b".into()));
    }

    #[test]
    fn array_get_out_of_bounds_returns_null() {
        let reg = NativeRegistry::collect();
        let entry = reg.get("std.array.get").unwrap();
        let arr = quew_runtime::value::Value::Array(vec![quew_runtime::value::Value::Number(1)]);
        let result = match &entry.handler {
            quew_runtime::native::NativeHandler::Sync(f) => {
                f(&[arr, quew_runtime::value::Value::Number(5)]).unwrap()
            }
        };
        assert_eq!(result, quew_runtime::value::Value::Null);
    }

    #[test]
    fn array_push_dispatch() {
        let reg = NativeRegistry::collect();
        let entry = reg.get("std.array.push").unwrap();
        let arr = quew_runtime::value::Value::Array(vec![
            quew_runtime::value::Value::Number(1),
            quew_runtime::value::Value::Number(2),
        ]);
        let result = match &entry.handler {
            quew_runtime::native::NativeHandler::Sync(f) => {
                f(&[arr, quew_runtime::value::Value::Number(3)]).unwrap()
            }
        };
        assert_eq!(
            result,
            quew_runtime::value::Value::Array(vec![
                quew_runtime::value::Value::Number(1),
                quew_runtime::value::Value::Number(2),
                quew_runtime::value::Value::Number(3),
            ])
        );
    }

    #[test]
    fn array_pop_dispatch() {
        let reg = NativeRegistry::collect();
        let entry = reg.get("std.array.pop").unwrap();
        let arr = quew_runtime::value::Value::Array(vec![
            quew_runtime::value::Value::Number(1),
            quew_runtime::value::Value::Number(2),
        ]);
        let result = match &entry.handler {
            quew_runtime::native::NativeHandler::Sync(f) => {
                f(&[arr]).unwrap()
            }
        };
        assert_eq!(result, quew_runtime::value::Value::Number(2));
    }

    #[test]
    fn array_pop_empty_returns_null() {
        let reg = NativeRegistry::collect();
        let entry = reg.get("std.array.pop").unwrap();
        let arr = quew_runtime::value::Value::Array(vec![]);
        let result = match &entry.handler {
            quew_runtime::native::NativeHandler::Sync(f) => {
                f(&[arr]).unwrap()
            }
        };
        assert_eq!(result, quew_runtime::value::Value::Null);
    }

    #[test]
    fn string_len_dispatch() {
        let reg = NativeRegistry::collect();
        let entry = reg.get("std.string.len").unwrap();
        let result = match &entry.handler {
            quew_runtime::native::NativeHandler::Sync(f) => {
                f(&[quew_runtime::value::Value::String("hello".into())]).unwrap()
            }
        };
        assert_eq!(result, quew_runtime::value::Value::Number(5));
    }

    #[test]
    fn string_contains_dispatch() {
        let reg = NativeRegistry::collect();
        let entry = reg.get("std.string.contains").unwrap();
        let result = match &entry.handler {
            quew_runtime::native::NativeHandler::Sync(f) => {
                f(&[
                    quew_runtime::value::Value::String("hello world".into()),
                    quew_runtime::value::Value::String("world".into()),
                ]).unwrap()
            }
        };
        assert_eq!(result, quew_runtime::value::Value::Bool(true));
    }
}
