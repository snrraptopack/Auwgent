pub use super::{Execution, ExecutionError};
pub use crate::value::Value;

mod utils;

mod assignment;
mod builtins;
mod control_flow;
mod functions;
mod is_type;
mod limits;
mod loops;
mod objects;
mod print;
mod strings;
