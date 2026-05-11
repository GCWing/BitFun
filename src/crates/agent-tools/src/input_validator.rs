use crate::ValidationResult;
use serde_json::Value;

pub struct InputValidator<'a> {
    input: &'a Value,
    result: ValidationResult,
}

impl<'a> InputValidator<'a> {
    pub fn new(input: &'a Value) -> Self {
        Self {
            input,
            result: ValidationResult::default(),
        }
    }

    /// Validate required fields (String type)
    pub fn validate_required(mut self, field: &str) -> Self {
        if self.result.result {
            self.result = validate_required_field(self.input, field);
        }
        self
    }

    /// Validate required enum fields (String type)
    pub fn validate_required_enum(mut self, field: &str, enum_values: &[&str]) -> Self {
        if self.result.result {
            self.result = validate_required_enum_field(self.input, field, enum_values);
        }
        self
    }

    /// Finish validation, return result
    pub fn finish(self) -> ValidationResult {
        self.result
    }
}

fn validate_required_enum_field(
    input: &Value,
    field: &str,
    enum_values: &[&str],
) -> ValidationResult {
    if let Some(value) = input.get(field).and_then(|v| v.as_str()) {
        if !enum_values.contains(&value) {
            return ValidationResult {
                result: false,
                message: Some(format!(
                    "{} must be one of {}",
                    field,
                    enum_values.join(", ")
                )),
                error_code: Some(400),
                meta: None,
            };
        }
    } else {
        return ValidationResult {
            result: false,
            message: Some(format!("{} is required", field)),
            error_code: Some(400),
            meta: None,
        };
    }
    ValidationResult {
        result: true,
        message: None,
        error_code: None,
        meta: None,
    }
}

fn validate_required_field(input: &Value, field: &str) -> ValidationResult {
    if let Some(value) = input.get(field).and_then(|v| v.as_str()) {
        if value.trim().is_empty() {
            return ValidationResult {
                result: false,
                message: Some(format!("{} cannot be empty", field)),
                error_code: Some(400),
                meta: None,
            };
        }
    } else {
        return ValidationResult {
            result: false,
            message: Some(format!("{} is required", field)),
            error_code: Some(400),
            meta: None,
        };
    }
    ValidationResult {
        result: true,
        message: None,
        error_code: None,
        meta: None,
    }
}
