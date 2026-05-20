use quew_macros::quew_builtin;

#[quew_builtin(
    id = "std.string.len",
    decl = r#"!@@function len(value: string): number"#,
)]
pub fn string_len(value: &str) -> i64 {
    value.len() as i64
}

#[quew_builtin(
    id = "std.string.is_empty",
    decl = r#"!@@function is_empty(value: string): bool"#,
)]
pub fn string_is_empty(value: &str) -> bool {
    value.is_empty()
}

#[quew_builtin(
    id = "std.string.contains",
    decl = r#"!@@function contains(value: string, needle: string): bool"#,
)]
pub fn string_contains(value: &str, needle: &str) -> bool {
    value.contains(needle)
}

#[quew_builtin(
    id = "std.string.starts_with",
    decl = r#"!@@function starts_with(value: string, prefix: string): bool"#,
)]
pub fn string_starts_with(value: &str, prefix: &str) -> bool {
    value.starts_with(prefix)
}

#[quew_builtin(
    id = "std.string.to_uppercase",
    decl = r#"!@@function to_uppercase(value: string): string"#,
)]
pub fn string_to_uppercase(value: &str) -> String {
    value.to_uppercase()
}
