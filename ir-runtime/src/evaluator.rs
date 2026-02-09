use crate::types::{AgentIR, Expression};
use serde_json::Value;
use std::collections::HashMap;
use std::error::Error;

pub struct Evaluator<'a> {
    pub ir: &'a AgentIR,
}

impl<'a> Evaluator<'a> {
    pub fn new(ir: &'a AgentIR) -> Self {
        Self { ir }
    }

    pub fn evaluate(
        &self,
        expr: &Expression,
        scope: &HashMap<String, Value>,
    ) -> Result<Value, Box<dyn Error>> {
        match expr {
            Expression::SchemaDirective { path } => {
                // Find the schema in the agent IR
                let schema = match path.as_str() {
                    "input" => &self.ir.input,
                    "output" => &self.ir.output,
                    "context" => &self.ir.context,
                    _ => return Err(format!("Unknown schema path: {}", path).into()),
                };

                if let Some(s) = schema {
                    Ok(Value::String(self.format_schema(s)))
                } else {
                    Ok(Value::String("{}".to_string()))
                }
            }
            Expression::Literal { value } => Ok(value.clone()),
            Expression::VarRef { value } => scope
                .get(value)
                .cloned()
                .ok_or_else(|| format!("Variable not found: {}", value).into()),
            Expression::MemberAccess { object, properties } => {
                // 1. Evaluate the base object (e.g., "input")
                let mut current = self.evaluate(object, scope)?;

                // 2. Traverse the properties (e.g., ".name")
                for prop in properties {
                    match current {
                        Value::Object(map) => {
                            if let Some(val) = map.get(prop) {
                                current = val.clone();
                            } else {
                                return Err(
                                    format!("Property '{}' not found in object", prop).into()
                                );
                            }
                        }
                        _ => {
                            return Err(
                                format!("Cannot access property '{}' on non-object", prop).into()
                            );
                        }
                    }
                }
                Ok(current)
            }
            Expression::Template { value } | Expression::Parts { value } => {
                // Evaluate all parts first
                let mut results = Vec::new();
                let mut all_strings = true;

                for part in value {
                    let val = self.evaluate(part, scope)?;
                    if !val.is_string() {
                        all_strings = false;
                    }
                    results.push((part, val));
                }

                // If all parts are strings, join them with smart whitespace handling
                if all_strings {
                    let joined = self.join_and_dedent(results);

                    // Only trim full result if it was a "Parts" expression (top-level prompt)
                    if matches!(expr, Expression::Parts { .. }) {
                        Ok(Value::String(joined.trim().to_string()))
                    } else {
                        Ok(Value::String(joined))
                    }
                } else {
                    // Otherwise return array of values
                    Ok(Value::Array(results.into_iter().map(|(_, v)| v).collect()))
                }
            }

            Expression::InlineIf {
                condition,
                then,
                else_block,
            } => {
                // 1. Evaluate left and right sides
                let left = self.evaluate(&condition.left, scope)?;
                let right = self.evaluate(&condition.right, scope)?;

                // 2. Perform comparison
                let is_true = match condition.operator.as_str() {
                    "==" => left == right,
                    "!=" => left != right,
                    // We can add >, <, etc. later if needed, but strings/numbers can be tricky in JSON
                    _ => {
                        return Err(format!("Operator {} not supported", condition.operator).into());
                    }
                };

                // 3. Choose which block to execute
                let block = if is_true { then } else { else_block };

                // 4. Evaluate the chosen block (similar to Parts/Template)
                let mut results = Vec::new();
                for part in block {
                    results.push((part, self.evaluate(part, scope)?));
                }

                // Join if strings (same logic as Template)
                if results.iter().all(|(_, v)| v.is_string()) {
                    let joined = self.join_and_dedent(results);
                    Ok(Value::String(joined.trim().to_string()))
                } else {
                    Ok(Value::Array(results.into_iter().map(|(_, v)| v).collect()))
                }
            }

            Expression::ContextRef { property } => {
                if let Some(Value::Object(ctx)) = scope.get("context") {
                    ctx.get(property)
                        .cloned()
                        .ok_or_else(|| format!("Context property '{}' not found", property).into())
                } else {
                    Err("Context object not found in scope".into())
                }
            }

            Expression::If {
                condition,
                then,
                else_block,
            } => {
                // Condition is an enum (Comparison or Boolean), so we need to extract it
                let (left_expr, right_expr, op) = match condition {
                    crate::types::Condition::Comparison(cmp) => {
                        (&cmp.left, &cmp.right, &cmp.operator)
                    }
                    crate::types::Condition::Boolean { value } => {
                        // For now, treat boolean as "value == true"
                        (
                            value,
                            &Box::new(Expression::Literal {
                                value: Value::Bool(true),
                            }),
                            &"==".to_string(),
                        )
                    }
                };

                // 1. Evaluate left and right sides
                let left = self.evaluate(left_expr, scope)?;
                let right = self.evaluate(right_expr, scope)?;

                // 2. Reuse comparison logic
                let is_true = match op.as_str() {
                    "==" => left == right,
                    "!=" => left != right,
                    _ => return Err(format!("Operator {} not supported", op).into()),
                };

                // 3. Execute block
                let block = if is_true { then } else { else_block };

                // Simple block execution
                let mut last_result = Value::Null;
                for stmt in block {
                    last_result = self.evaluate(stmt, scope)?;
                }
                Ok(last_result)
            }

            Expression::Return { value } => self.evaluate(value, scope),
            Expression::Expression { value } => self.evaluate(value, scope),
        }
    }

    // Helper method for smart dedent logic
    fn join_and_dedent(&self, parts: Vec<(&Expression, Value)>) -> String {
        let mut joined = String::new();
        for (expr, val) in parts {
            let s = val.as_str().unwrap();

            // Smart Dedent Logic:
            // If we are about to append a block result (InlineIf or If),
            // and the buffer ends with whitespace (indentation), trim it.
            if matches!(expr, Expression::InlineIf { .. } | Expression::If { .. }) {
                let trimmed = joined.trim_end_matches(|c| c == ' ' || c == '\t');
                let len = trimmed.len();
                joined.truncate(len);
            }

            joined.push_str(s);
        }
        joined
    }

    fn format_schema(&self, schema: &Value) -> String {
        if let Some(obj) = schema.as_object() {
            let mut fields = Vec::new();
            for (name, def) in obj {
                let is_optional = def["optional"].as_bool().unwrap_or(false);
                let name_tag = if is_optional {
                    format!("{}?", name)
                } else {
                    name.clone()
                };
                let field_type = def["type"].as_str().unwrap_or("any");

                let mut field_str = format!("{}:{}", name_tag, field_type);
                if let Some(desc) = def["description"].as_str() {
                    field_str.push_str(" // ");
                    field_str.push_str(desc);
                }
                fields.push(field_str);
            }
            format!("schema: {{ {} }}", fields.join(", "))
        } else {
            "{}".to_string()
        }
    }
}
