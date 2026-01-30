use anyhow::Result;
use chrono::{DateTime, Utc, FixedOffset};
use serde_json::{json, Value};

pub struct Tools;

impl Tools {
    pub fn get_tools_definition() -> Vec<Value> {
        vec![
            json!({
                "type": "function",
                "function": {
                    "name": "get_current_date",
                    "description": "Get the current date in various formats. Useful for answering questions about what day it is, what date it is, etc.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "format": {
                                "type": "string",
                                "enum": ["iso", "readable", "day_of_week", "day_name", "full"],
                                "description": "Format for the date: 'iso' (YYYY-MM-DD), 'readable' (Month Day, Year), 'day_of_week' (Monday, Tuesday, etc.), 'day_name' (just the day name), 'full' (full date and time)"
                            },
                            "timezone": {
                                "type": "string",
                                "description": "Optional timezone (e.g., 'UTC', 'America/New_York', 'Europe/London'). Defaults to UTC."
                            }
                        }
                    }
                }
            }),
            json!({
                "type": "function",
                "function": {
                    "name": "get_current_time",
                    "description": "Get the current time in various formats. Useful for answering questions about what time it is.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "format": {
                                "type": "string",
                                "enum": ["12h", "24h", "iso", "timestamp"],
                                "description": "Format: '12h' (12-hour with AM/PM), '24h' (24-hour), 'iso' (ISO 8601), 'timestamp' (Unix timestamp)"
                            },
                            "timezone": {
                                "type": "string",
                                "description": "Optional timezone (e.g., 'UTC', 'America/New_York'). Defaults to UTC."
                            }
                        }
                    }
                }
            }),
            json!({
                "type": "function",
                "function": {
                    "name": "calculate",
                    "description": "Perform mathematical calculations. Supports basic arithmetic, percentages, and common math operations.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "expression": {
                                "type": "string",
                                "description": "Mathematical expression to evaluate (e.g., '2 + 2', '100 * 0.15', 'sqrt(16)', 'pow(2, 8)')"
                            }
                        },
                        "required": ["expression"]
                    }
                }
            }),
            json!({
                "type": "function",
                "function": {
                    "name": "format_date",
                    "description": "Format a date string into different formats. Useful for converting between date formats.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "date": {
                                "type": "string",
                                "description": "Date string to format (various formats accepted)"
                            },
                            "output_format": {
                                "type": "string",
                                "enum": ["iso", "readable", "timestamp", "relative"],
                                "description": "Desired output format"
                            }
                        },
                        "required": ["date"]
                    }
                }
            }),
            json!({
                "type": "function",
                "function": {
                    "name": "timezone_convert",
                    "description": "Convert a time from one timezone to another.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "time": {
                                "type": "string",
                                "description": "Time string to convert"
                            },
                            "from_timezone": {
                                "type": "string",
                                "description": "Source timezone (e.g., 'UTC', 'America/New_York')"
                            },
                            "to_timezone": {
                                "type": "string",
                                "description": "Target timezone (e.g., 'UTC', 'Europe/London')"
                            }
                        },
                        "required": ["time", "from_timezone", "to_timezone"]
                    }
                }
            }),
            json!({
                "type": "function",
                "function": {
                    "name": "generate_uuid",
                    "description": "Generate a UUID (Universally Unique Identifier). Useful for generating unique IDs.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "version": {
                                "type": "string",
                                "enum": ["v4", "nil"],
                                "description": "UUID version: 'v4' (random) or 'nil' (all zeros)"
                            }
                        }
                    }
                }
            }),
            json!({
                "type": "function",
                "function": {
                    "name": "hash_string",
                    "description": "Generate a hash of a string using various algorithms. Useful for data integrity checks.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "text": {
                                "type": "string",
                                "description": "Text to hash"
                            },
                            "algorithm": {
                                "type": "string",
                                "enum": ["md5", "sha256", "sha512"],
                                "description": "Hash algorithm to use"
                            }
                        },
                        "required": ["text", "algorithm"]
                    }
                }
            }),
            json!({
                "type": "function",
                "function": {
                    "name": "base64_encode",
                    "description": "Encode a string to Base64 format.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "text": {
                                "type": "string",
                                "description": "Text to encode"
                            }
                        },
                        "required": ["text"]
                    }
                }
            }),
            json!({
                "type": "function",
                "function": {
                    "name": "base64_decode",
                    "description": "Decode a Base64 string.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "text": {
                                "type": "string",
                                "description": "Base64 string to decode"
                            }
                        },
                        "required": ["text"]
                    }
                }
            }),
        ]
    }

    pub fn execute_tool(name: &str, arguments: &Value) -> Result<String> {
        match name {
            "get_current_date" => Self::get_current_date(arguments),
            "get_current_time" => Self::get_current_time(arguments),
            "calculate" => Self::calculate(arguments),
            "format_date" => Self::format_date(arguments),
            "timezone_convert" => Self::timezone_convert(arguments),
            "generate_uuid" => Self::generate_uuid(arguments),
            "hash_string" => Self::hash_string(arguments),
            "base64_encode" => Self::base64_encode(arguments),
            "base64_decode" => Self::base64_decode(arguments),
            _ => Err(anyhow::anyhow!("Unknown tool: {}", name)),
        }
    }

    fn get_current_date(args: &Value) -> Result<String> {
        let format = args.get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("readable");
        
        let timezone_str = args.get("timezone")
            .and_then(|v| v.as_str())
            .unwrap_or("UTC");
        
        let now = if timezone_str == "UTC" {
            Utc::now()
        } else {
            // For simplicity, use UTC and note timezone in output
            Utc::now()
        };
        
        let result = match format {
            "iso" => now.format("%Y-%m-%d").to_string(),
            "readable" => now.format("%B %d, %Y").to_string(),
            "day_of_week" | "day_name" => now.format("%A").to_string(),
            "full" => {
                if timezone_str == "UTC" {
                    now.format("%A, %B %d, %Y at %H:%M:%S UTC").to_string()
                } else {
                    format!("{} (timezone: {})", now.format("%A, %B %d, %Y at %H:%M:%S UTC"), timezone_str)
                }
            },
            _ => now.format("%B %d, %Y").to_string(),
        };
        
        Ok(result)
    }

    fn get_current_time(args: &Value) -> Result<String> {
        let format = args.get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("24h");
        
        let timezone_str = args.get("timezone")
            .and_then(|v| v.as_str())
            .unwrap_or("UTC");
        
        let now = Utc::now();
        
        let result = match format {
            "12h" => now.format("%I:%M:%S %p UTC").to_string(),
            "24h" => now.format("%H:%M:%S UTC").to_string(),
            "iso" => now.to_rfc3339(),
            "timestamp" => now.timestamp().to_string(),
            _ => now.format("%H:%M:%S UTC").to_string(),
        };
        
        if timezone_str != "UTC" {
            Ok(format!("{} (requested timezone: {})", result, timezone_str))
        } else {
            Ok(result)
        }
    }

    fn calculate(args: &Value) -> Result<String> {
        let expression = args.get("expression")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'expression' parameter"))?;
        
        // Simple math evaluation (for production, use a proper math parser)
        // This is a basic implementation - consider using meval or similar for production
        let result = Self::eval_math(expression)?;
        Ok(result.to_string())
    }

    fn eval_math(expr: &str) -> Result<f64> {
        // Use meval for proper math evaluation
        meval::eval_str(expr)
            .map_err(|e| anyhow::anyhow!("Math evaluation error: {}", e))
    }

    fn format_date(args: &Value) -> Result<String> {
        let date_str = args.get("date")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'date' parameter"))?;
        
        let output_format = args.get("output_format")
            .and_then(|v| v.as_str())
            .unwrap_or("readable");
        
        // Try to parse the date
        let dt = DateTime::parse_from_rfc3339(date_str)
            .or_else(|_| {
                // Try other formats
                date_str.parse::<DateTime<Utc>>()
                    .map(|dt| dt.with_timezone(&FixedOffset::east_opt(0).unwrap()))
            })
            .or_else(|_| {
                // Try timestamp
                date_str.parse::<i64>()
                    .map(|ts| DateTime::from_timestamp(ts, 0).unwrap().with_timezone(&FixedOffset::east_opt(0).unwrap()))
            })?;
        
        let result = match output_format {
            "iso" => dt.format("%Y-%m-%dT%H:%M:%S%z").to_string(),
            "readable" => dt.format("%B %d, %Y at %H:%M:%S").to_string(),
            "timestamp" => dt.timestamp().to_string(),
            "relative" => {
                let now = Utc::now();
                let diff = now - dt.with_timezone(&Utc);
                if diff.num_days() > 0 {
                    format!("{} days ago", diff.num_days())
                } else if diff.num_hours() > 0 {
                    format!("{} hours ago", diff.num_hours())
                } else if diff.num_minutes() > 0 {
                    format!("{} minutes ago", diff.num_minutes())
                } else {
                    "just now".to_string()
                }
            },
            _ => dt.format("%B %d, %Y").to_string(),
        };
        
        Ok(result)
    }

    fn timezone_convert(args: &Value) -> Result<String> {
        // Simplified - for production, use chrono-tz
        let time_str = args.get("time")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'time' parameter"))?;
        
        let _from = args.get("from_timezone")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'from_timezone' parameter"))?;
        
        let _to = args.get("to_timezone")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'to_timezone' parameter"))?;
        
        // Parse and convert (simplified - use chrono-tz for full support)
        let dt = DateTime::parse_from_rfc3339(time_str)
            .or_else(|_| time_str.parse::<DateTime<Utc>>().map(|dt| dt.with_timezone(&FixedOffset::east_opt(0).unwrap())))?;
        
        Ok(format!("Converted time: {} (Note: Full timezone conversion requires chrono-tz library)", dt.format("%Y-%m-%d %H:%M:%S")))
    }

    fn generate_uuid(args: &Value) -> Result<String> {
        let version = args.get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("v4");
        
        match version {
            "v4" => {
                use uuid::Uuid;
                Ok(Uuid::new_v4().to_string())
            },
            "nil" => {
                use uuid::Uuid;
                Ok(Uuid::nil().to_string())
            },
            _ => Err(anyhow::anyhow!("Invalid UUID version")),
        }
    }

    fn hash_string(args: &Value) -> Result<String> {
        let text = args.get("text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'text' parameter"))?;
        
        let algorithm = args.get("algorithm")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'algorithm' parameter"))?;
        
        match algorithm {
            "md5" => {
                let digest = md5::compute(text.as_bytes());
                Ok(format!("{:x}", digest))
            },
            "sha256" => {
                use sha2::Sha256;
                use digest::Digest;
                let mut hasher = Sha256::new();
                hasher.update(text.as_bytes());
                Ok(format!("{:x}", hasher.finalize()))
            },
            "sha512" => {
                use sha2::Sha512;
                use digest::Digest;
                let mut hasher = Sha512::new();
                hasher.update(text.as_bytes());
                Ok(format!("{:x}", hasher.finalize()))
            },
            _ => Err(anyhow::anyhow!("Unsupported algorithm")),
        }
    }

    fn base64_encode(args: &Value) -> Result<String> {
        let text = args.get("text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'text' parameter"))?;
        
        use base64::{Engine as _, engine::general_purpose};
        Ok(general_purpose::STANDARD.encode(text.as_bytes()))
    }

    fn base64_decode(args: &Value) -> Result<String> {
        let text = args.get("text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'text' parameter"))?;
        
        use base64::{Engine as _, engine::general_purpose};
        let decoded = general_purpose::STANDARD.decode(text)?;
        Ok(String::from_utf8(decoded)?)
    }
}
