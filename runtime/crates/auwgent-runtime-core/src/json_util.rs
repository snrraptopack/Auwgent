use serde_json::Value;

/// Deep-merge two JSON objects. `b` wins on conflicts.
/// Non-object values are replaced outright.
pub fn deep_merge_json(a: Value, b: Value) -> Value {
    match (a, b) {
        (Value::Object(mut a_obj), Value::Object(b_obj)) => {
            for (k, v) in b_obj {
                let entry = a_obj.entry(k).or_insert_with(|| Value::Null);
                *entry = deep_merge_json(entry.clone(), v);
            }
            Value::Object(a_obj)
        }
        (_, b) => b,
    }
}
