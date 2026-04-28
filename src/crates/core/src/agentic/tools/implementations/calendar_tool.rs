use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext};
use crate::util::errors::{BitFunError, BitFunResult};
use crate::util::JS_THREADSAFE_FUNCTIONS;
use async_trait::async_trait;
use serde_json::{ Value,json};

pub struct CalendarTool;

impl CalendarTool {
    pub fn new() -> CalendarTool {
        Self
    }
}

#[async_trait]
impl Tool for CalendarTool {
    fn name(&self) -> &str {
        "Calendar"
    }

    async fn description(&self) -> BitFunResult<String> {
        Ok(r#"Manages all types of calendar schedules, including events, reminders, deadlines, and all-day entries.

        Usage Guidelines:
        - Supported actions: 'create' (new entry)
        - You MUST extract the specific city or venue into the 'location' field (e.g, 'Beijing').
        - DO NOT leave the primary location only inside the 'description' or 'title'.
        - Time Format: Always use 'YYYY-MM-DD HH:mm'.
        - Participants can include names or email address, Leave empty for personal tasks.
        "#.to_string())
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "Short title of the schedule (e.g., 'Flight to Tokyo'. 'Dentist Appointment')",
                },
                "description": {
                    "type": "string",
                    "description": "Detailed notes or additional information"
                },
                "start_time": {
                    "type": "string",
                    "description": "YYYY-MM-DD HH:mm format"
                },
                "end_time": {
                    "type": "string",
                    "description": "YYYY-MM-DD HH:mm"
                },
                "location": {
                    "type": "string",
                    "description": "The specific physical location, city, or address. E.G., 'Beijing' or 'Forbidden City'",
                }
            },
            "required": ["action"],
            "additionalProperties": false
        })
    }

    fn is_readonly(&self) -> bool {
        false
    }

    fn is_concurrency_safe(&self, _input: Option<&Value>) -> bool {
        false
    }

    async fn call_impl(
        &self,
        input: &Value,
        _context: &ToolUseContext
    )-> BitFunResult<Vec<ToolResult>> {
        let title = input.get("title").and_then(|v| v.as_str()).unwrap_or_default();
        let description = input.get("description").and_then(|v| v.as_str()).unwrap_or_default();
        let start_time = input.get("start_time").and_then(|v| v.as_str()).unwrap_or_default();
        let end_time = input.get("end_time").and_then(|v| v.as_str()).unwrap_or_default();
        let info = CalendarInfo::new(title.to_string(), description.to_string(), start_time.to_string(), end_time.to_string());

        let res = call_calender(serde_json::to_string(&info).unwrap_or_default());
        let action = "创建日程";

        let result = ToolResult::Result {
            data: json!({
                "action": action,
                "success": true
            }),
            result_for_assistant: Some(format!(
                "Calendar {} operation executed successfully",
                action
            )),
            image_attachments: None,
        };
        Ok(vec![result])
    }
}

#[napi(object)]
#[derive(Debug,Clone,Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarInfo {
    pub title: String,
    pub start_time: String,
    pub end_time: String,
    pub description: String,
}

impl CalendarInfo {
    pub fn new(title: String, start_time: String, end_time: String, description: String) -> Self {
        Self {
            title,
            start_time,
            end_time,
            description
        }
    }
}

use napi_derive_ohos::napi;
use serde::Serialize;

use napi_ohos::threadsafe_function::ThreadsafeFunctionCallMode;

pub fn call_calender(args: String) -> Result<String, String>{
    let result = Ok(args);
    match JS_THREADSAFE_FUNCTIONS.write().get("call_calender") {
        None => {
            return Err("The Arkts has not register the functions".to_string());
        }
        Some(functions) => {
            functions.call_with_return_value(
                result,
                ThreadsafeFunctionCallMode::Blocking,
                move |result,_| {
                    match result {
                        Ok(_) => {
                            log::info!("Successfully called Arkts");
                        }
                        Err(err) => {
                            log::error!("call calender with error {:?}", err);
                        }
                    }
                    Ok(())
                }
            );
        }
    }
    Ok("".to_string())
}