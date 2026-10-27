//! The MCP server "pc-health" for Claude: `vitals.exe --mcp`, JSON-RPC 2.0 over stdin and stdout, a line per message.
//! The tools only read: the health of the PC, the events, the disks, what could be cleaned, the updates, the network.
//! Nothing is deleted or changed through it (the brief, §2: actions run only in the window).

#[cfg(windows)]
pub mod tools;

use std::io::{BufRead, Write};

use serde_json::{Value, json};

pub const FLAG: &str = "--mcp";
const SERVER_NAME: &str = "pc-health";
/// The versions this server speaks, the newest first; a client asking another gets the newest
const VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

const PARSE_ERROR: i64 = -32700;
const NO_METHOD: i64 = -32601;
const BAD_PARAMS: i64 = -32602;

/// What a tool needs: its name, what it says to the model, the schema of its arguments.
#[derive(Debug, Clone, Copy)]
pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub schema: fn() -> Value,
}

fn no_arguments() -> Value {
    json!({ "type": "object", "properties": {}, "additionalProperties": false })
}

fn events_arguments() -> Value {
    json!({
        "type": "object",
        "properties": {
            "days": { "type": "integer", "minimum": 1, "maximum": 366, "description": "How many days back, today included; 7 by default" },
            "kind": {
                "type": "string",
                "enum": ["crash", "bsod", "gpuReset", "nvContainer", "whea", "iommu", "appCrash", "error"],
                "description": "Only this kind; every kind when left out"
            }
        },
        "additionalProperties": false
    })
}

fn days_argument() -> Value {
    json!({
        "type": "object",
        "properties": {
            "days": { "type": "integer", "minimum": 1, "maximum": 366, "description": "The period in days, today included; 30 by default" }
        },
        "additionalProperties": false
    })
}

pub const TOOLS: [ToolSpec; 6] = [
    ToolSpec {
        name: "health_summary",
        description: "Days without a crash, free space on the system drive, and the counts of unexpected restarts, BSODs, WHEA errors, GPU driver resets and application crashes per day over a period.",
        schema: days_argument,
    },
    ToolSpec {
        name: "events",
        description: "The Windows events the panel keeps (crashes, BSODs with their bugcheck code, WHEA, GPU resets, NVIDIA service crashes, IOMMU, application crashes, other System errors), newest first, at most 50.",
        schema: events_arguments,
    },
    ToolSpec {
        name: "disk_usage",
        description: "The volumes with used and free space, the physical disks with health, temperature and wear, and the totals of the last scan of the home folder.",
        schema: no_arguments,
    },
    ToolSpec {
        name: "cleanup_preview",
        description: "What the last scan found to clean, grouped as safe (a whitelist) or review first, with sizes and paths. Preview only: deleting is done by the user in the Vitals window.",
        schema: no_arguments,
    },
    ToolSpec {
        name: "updates_list",
        description: "The programs winget can upgrade, with the installed and the available version and whether the program is open now. Takes up to a minute: winget asks its sources.",
        schema: no_arguments,
    },
    ToolSpec {
        name: "network_status",
        description: "The network adapters with their link speed and addresses, and the latency, loss and jitter to the router and to the internet over the last day.",
        schema: no_arguments,
    },
];

/// The tools of a running server; the tests give a fake.
pub trait Tools {
    fn call(&self, name: &str, arguments: &Value) -> Result<Value, String>;
}

fn reply(id: &Value, result: &Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn failure(id: &Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn tool_list() -> Value {
    let tools: Vec<Value> = TOOLS
        .iter()
        .map(|tool| {
            json!({
                "name": tool.name,
                "description": tool.description,
                "inputSchema": (tool.schema)(),
                "annotations": { "readOnlyHint": true, "openWorldHint": false }
            })
        })
        .collect();
    json!({ "tools": tools })
}

/// The answer to one message; none for a notification.
pub fn respond(message: &Value, tools: &impl Tools) -> Option<Value> {
    let id = message.get("id")?.clone();
    let method = message.get("method").and_then(Value::as_str).unwrap_or("");
    let params = message.get("params").cloned().unwrap_or(Value::Null);
    Some(match method {
        "initialize" => {
            let asked = params.get("protocolVersion").and_then(Value::as_str);
            let version = asked
                .filter(|asked| VERSIONS.contains(asked))
                .unwrap_or(VERSIONS[0]);
            reply(
                &id,
                &json!({
                    "protocolVersion": version,
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": SERVER_NAME, "version": env!("CARGO_PKG_VERSION") },
                    "instructions": "Read-only health of this Windows PC from the Vitals panel. Nothing here deletes or changes anything."
                }),
            )
        }
        "ping" => reply(&id, &json!({})),
        "tools/list" => reply(&id, &tool_list()),
        "tools/call" => {
            let Some(name) = params.get("name").and_then(Value::as_str) else {
                return Some(failure(&id, BAD_PARAMS, "a tool call names its tool"));
            };
            if !TOOLS.iter().any(|tool| tool.name == name) {
                return Some(failure(&id, BAD_PARAMS, &format!("no tool {name}")));
            }
            let arguments = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            match tools.call(name, &arguments) {
                Ok(value) => reply(
                    &id,
                    &json!({
                        "content": [{ "type": "text", "text": value.to_string() }],
                        "structuredContent": value,
                        "isError": false
                    }),
                ),
                Err(message) => reply(
                    &id,
                    &json!({ "content": [{ "type": "text", "text": message }], "isError": true }),
                ),
            }
        }
        _ => failure(&id, NO_METHOD, &format!("no method {method}")),
    })
}

/// Answers every line of `input` until it ends; a line that is not JSON gets a parse error.
pub fn serve(
    input: impl BufRead,
    mut output: impl Write,
    tools: &impl Tools,
) -> std::io::Result<()> {
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let answer = match serde_json::from_str::<Value>(&line) {
            Ok(message) => respond(&message, tools),
            Err(error) => Some(failure(&Value::Null, PARSE_ERROR, &error.to_string())),
        };
        if let Some(answer) = answer {
            writeln!(output, "{answer}")?;
            output.flush()?;
        }
    }
    Ok(())
}

/// `days` of the arguments, within its bounds, else the default.
pub fn days_of(arguments: &Value, default: u32) -> u32 {
    arguments
        .get("days")
        .and_then(Value::as_u64)
        .and_then(|days| u32::try_from(days).ok())
        .map_or(default, |days| days.clamp(1, vitals_ipc::MAX_RANGE_DAYS))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake;

    impl Tools for Fake {
        fn call(&self, name: &str, arguments: &Value) -> Result<Value, String> {
            match name {
                "health_summary" => {
                    Ok(json!({ "daysWithoutCrash": 3, "days": days_of(arguments, 30) }))
                }
                _ => Err("winget is not installed".to_owned()),
            }
        }
    }

    fn answers(lines: &str) -> Vec<Value> {
        let mut output = Vec::new();
        serve(lines.as_bytes(), &mut output, &Fake).unwrap();
        String::from_utf8(output)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    #[test]
    fn a_session_initializes_lists_and_calls() {
        let replies = answers(concat!(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{}}}"#,
            "\n",
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"health_summary","arguments":{"days":900}}}"#,
            "\n"
        ));
        assert_eq!(replies.len(), 3, "a notification gets no answer");
        assert_eq!(replies[0]["result"]["protocolVersion"], "2025-03-26");
        assert_eq!(replies[0]["result"]["serverInfo"]["name"], "pc-health");
        assert_eq!(
            replies[1]["result"]["tools"].as_array().unwrap().len(),
            TOOLS.len()
        );
        assert_eq!(
            replies[1]["result"]["tools"][0]["annotations"]["readOnlyHint"],
            true
        );
        assert_eq!(replies[2]["result"]["structuredContent"]["days"], 366);
        assert_eq!(replies[2]["result"]["isError"], false);
    }

    #[test]
    fn a_failed_tool_an_unknown_one_and_garbage_are_told_apart() {
        let replies = answers(concat!(
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"updates_list"}}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"cleanup_run"}}"#,
            "\n",
            "not json\n",
            r#"{"jsonrpc":"2.0","id":4,"method":"resources/list"}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":5,"method":"initialize","params":{"protocolVersion":"1999-01-01"}}"#,
            "\n"
        ));
        assert_eq!(replies[0]["result"]["isError"], true);
        assert_eq!(replies[1]["error"]["code"], BAD_PARAMS);
        assert_eq!(replies[2]["error"]["code"], PARSE_ERROR);
        assert_eq!(replies[3]["error"]["code"], NO_METHOD);
        assert_eq!(replies[4]["result"]["protocolVersion"], VERSIONS[0]);
    }

    #[test]
    fn no_tool_deletes_or_changes() {
        for tool in TOOLS {
            assert!(
                !tool.name.contains("run")
                    && !tool.name.contains("set")
                    && !tool.name.contains("delete")
            );
        }
    }
}
