use crate::main_agent::{
  AuwgentTools,
  AuwgentConfig,
  GetLocationResult,
  GetMarksArgs,
  GetMarksResult,
  AuwgentContext,
  AuwgentApiKeys,
  AuwgentMiddlewareRegistry
};

struct Tools;

impl AuwgentTools for Tools {

    fn get_location(&self) -> GetLocationResult {
        "Tarkwa".to_string()
    }

    fn get_marks(&self,_args:GetMarksArgs) -> GetMarksResult {
        "A,B,C,D".to_string()
    }
}


pub fn get_agent_config(middleware: Vec<AuwgentMiddlewareRegistry>) -> AuwgentConfig {
  AuwgentConfig {
    middleware: middleware,
    context: AuwgentContext{
        user_name: "Amihere".to_string(),
        age: 25.4,
        id: "10".to_string(),
    },
    api_keys: AuwgentApiKeys::default(),
    tools: Tools.into()
  }
}
