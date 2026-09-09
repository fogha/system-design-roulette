//! Runner adapters adapted from Remote Ledger; learning validation stays in Generator.
use super::{process, types::*, Runner};
use crate::generator::{GenError, Result};
use serde_json::{json, Value};
use tokio::process::Command;

pub struct AdapterResult {
    pub text: String,
    pub model: String,
    pub usage: Usage,
    pub tool_calls: Vec<ToolCall>,
}

fn merged_prompt(req: &RunRequest) -> String {
    let mut prompt = req
        .system
        .as_ref()
        .map(|s| format!("{s}\n\n"))
        .unwrap_or_default();
    prompt.push_str(&req.prompt);
    prompt
}
fn tokens(value: Option<u64>, text: &str) -> u64 {
    value.unwrap_or_else(|| (text.chars().count() as u64).div_ceil(4))
}
fn estimate(req: &RunRequest, text: &str) -> Usage {
    Usage {
        input_tokens: tokens(None, &merged_prompt(req)),
        output_tokens: tokens(None, text),
        cached_tokens: 0,
        cost_usd: Some(0.0),
        metered: false,
        tokens_estimated: true,
    }
}

pub async fn run_cli(runner: &Runner, req: &RunRequest, call_id: &str) -> Result<AdapterResult> {
    let scratch = process::Scratch::new(&runner.scratch_dir, call_id)?;
    let id = req.route.runner;
    let mut command = if id == RunnerId::CustomCli {
        let words = process::command_words(&req.route.custom_command)?;
        let mut command = Command::new(process::resolve(&words[0]).ok_or(GenError::NoBinary)?);
        let prompt = merged_prompt(req);
        let mut substituted = false;
        for arg in &words[1..] {
            if arg.contains("{prompt}") {
                substituted = true;
                command.arg(arg.replace("{prompt}", &prompt));
            } else {
                command.arg(arg);
            }
        }
        if !substituted {
            command.arg(prompt);
        }
        command
    } else {
        Command::new(runner.binary(id).ok_or(GenError::NoBinary)?)
    };
    let model = req.route.model.as_str();
    let model_flag = model != "default" && !model.is_empty();
    let prompt = merged_prompt(req);
    let mut input = None;
    let out_file = scratch.0.join("last-message.txt");
    match id {
        RunnerId::ClaudeCli => {
            command.args([
                "-p",
                "--output-format",
                "stream-json",
                "--verbose",
                "--max-turns",
                "25",
            ]);
            if model_flag {
                command.args(["--model", model]);
            }
            if let Some(system) = &req.system {
                command.args(["--append-system-prompt", system]);
            }
            if req.allow_web {
                command.args(["--allowedTools", "WebSearch,WebFetch"]);
            }
            command.args(["--disallowedTools", "Bash,Edit,Write,NotebookEdit"]);
            input = Some(req.prompt.as_str());
        }
        RunnerId::CodexCli => {
            command
                .args([
                    "exec",
                    "--skip-git-repo-check",
                    "--color",
                    "never",
                    "--sandbox",
                    "read-only",
                    "--json",
                    "--output-last-message",
                ])
                .arg(&out_file);
            if model_flag {
                command.args(["--model", model]);
            }
            command.arg("-");
            input = Some(prompt.as_str());
        }
        RunnerId::CursorCli => {
            command.args(["-p", "--output-format", "text"]);
            if model_flag {
                command.args(["--model", model]);
            }
            command.arg(&prompt);
        }
        RunnerId::GeminiCli => {
            command.arg("-p").arg(&prompt);
            if model_flag {
                command.args(["--model", model]);
            }
        }
        RunnerId::CustomCli => {}
        _ => unreachable!("API runner passed to CLI transport"),
    }
    command.current_dir(&scratch.0);
    let on_event = |line: &str| {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            return;
        };
        match event["type"].as_str() {
            Some("assistant") => {
                for block in event["message"]["content"].as_array().into_iter().flatten() {
                    match block["type"].as_str() {
                        Some("tool_use") => runner.log(format!(
                            "{} · tool {}",
                            id.label(),
                            block["name"].as_str().unwrap_or("request")
                        )),
                        Some("text") => runner.log(format!("{} · drafting", id.label())),
                        _ => {}
                    }
                }
            }
            Some("turn.started") => runner.log(format!("{} · working", id.label())),
            _ => {}
        }
    };
    let raw = process::capture_events(&mut command, input, req.timeout, Some(&on_event)).await?;
    let mut result = AdapterResult {
        text: raw.clone(),
        model: model.into(),
        usage: estimate(req, &raw),
        tool_calls: vec![],
    };
    if id == RunnerId::ClaudeCli {
        let envelope = raw
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .find(|v| v["type"] == "result" || v.get("result").is_some())
            .ok_or_else(|| GenError::Parse("Claude returned no final result envelope".into()))?;
        if envelope["is_error"] == true
            || envelope["subtype"]
                .as_str()
                .is_some_and(|v| v.starts_with("error"))
        {
            return Err(GenError::Api(
                "Claude ended with an error; check CLI authentication, quota and turn limit".into(),
            ));
        }
        result.text = envelope["result"].as_str().unwrap_or_default().into();
        result.model = envelope["model"].as_str().unwrap_or(model).into();
        let u = &envelope["usage"];
        let input = u["input_tokens"].as_u64();
        let output = u["output_tokens"].as_u64();
        let cached = u["cache_read_input_tokens"].as_u64().unwrap_or(0);
        let created = u["cache_creation_input_tokens"].as_u64().unwrap_or(0);
        result.usage = Usage {
            input_tokens: tokens(input, &prompt) + cached + created,
            output_tokens: tokens(output, &result.text),
            cached_tokens: cached,
            cost_usd: envelope["total_cost_usd"]
                .as_f64()
                .filter(|n| n.is_finite() && *n >= 0.0),
            metered: false,
            tokens_estimated: input.is_none() || output.is_none(),
        };
    } else if id == RunnerId::CodexCli {
        // A call owns its directory. Never consume logs or another call's last answer.
        result.text = std::fs::read_to_string(&out_file)
            .map_err(|_| GenError::Parse("Codex returned no final message file".into()))?;
        for event in raw
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        {
            if event["type"] == "turn.completed" {
                let u = &event["usage"];
                result.usage = Usage {
                    input_tokens: tokens(u["input_tokens"].as_u64(), &prompt),
                    output_tokens: tokens(u["output_tokens"].as_u64(), &result.text),
                    cached_tokens: u["cached_input_tokens"].as_u64().unwrap_or(0),
                    cost_usd: Some(0.0),
                    metered: false,
                    tokens_estimated: u["input_tokens"].as_u64().is_none()
                        || u["output_tokens"].as_u64().is_none(),
                };
            }
        }
    }
    if result.text.trim().is_empty() {
        return Err(GenError::Parse(format!(
            "{} returned an empty answer",
            id.label()
        )));
    }
    Ok(result)
}

pub fn to_wire(message: &ChatMessage) -> Value {
    if message.role == "tool" {
        return json!({"role":"tool", "tool_call_id":message.tool_call_id, "content":message.content});
    }
    let mut result = json!({"role":message.role, "content":message.content});
    if !message.tool_calls.is_empty() {
        result["tool_calls"] = json!(message.tool_calls.iter().map(|call| json!({"id":call.id,"type":"function","function":{"name":call.name,"arguments":call.args.to_string()}})).collect::<Vec<_>>());
    }
    result
}

pub fn deepseek_body(req: &RunRequest) -> Value {
    let mut messages: Vec<_> = req.messages.iter().map(to_wire).collect();
    if messages.is_empty() {
        if let Some(system) = &req.system {
            messages.push(json!({"role":"system","content":system}));
        }
        let mut prompt = req.prompt.clone();
        if req.allow_web {
            prompt.push_str("\nYou have no provider-side live browsing in this call. Use only source material actually retrieved by the app; never invent URLs.");
        }
        if req.json {
            prompt.push_str("\nReturn the requested JSON object, without markdown fences.");
        }
        messages.push(json!({"role":"user","content":prompt}));
    }
    let mut body = json!({"model":req.route.model,"messages":messages,"thinking":{"type":"disabled"},"max_tokens":req.max_tokens,"stream":false});
    if !req.tools.is_empty() {
        body["tools"] = json!(req.tools.iter().map(|t| json!({"type":"function","function":{"name":t.name,"description":t.description,"parameters":t.parameters}})).collect::<Vec<_>>());
        body["tool_choice"] = json!("auto");
    } else if req.json {
        body["response_format"] = json!({"type":"json_object"});
    }
    body
}

pub fn parse_compatible(value: Value, req: &RunRequest) -> Result<AdapterResult> {
    if value.get("error").is_some_and(|e| !e.is_null()) {
        return Err(GenError::Api("provider returned an error response".into()));
    }
    let message = &value["choices"][0]["message"];
    let text = message["content"].as_str().unwrap_or_default().to_string();
    let tool_calls: Vec<_> = message["tool_calls"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
        .filter_map(|(i, call)| {
            let name = call["function"]["name"].as_str()?.to_string();
            let args = call["function"]["arguments"]
                .as_str()
                .and_then(parse_json)
                .filter(Value::is_object)
                .unwrap_or_else(|| json!({}));
            Some(ToolCall {
                id: call["id"]
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("call_{i}")),
                name,
                args,
            })
        })
        .collect();
    if text.trim().is_empty() && tool_calls.is_empty() {
        return Err(GenError::Parse(
            "provider returned no answer or tool calls".into(),
        ));
    }
    let u = &value["usage"];
    let input = u["prompt_tokens"].as_u64();
    let output = u["completion_tokens"].as_u64();
    Ok(AdapterResult {
        model: value["model"].as_str().unwrap_or(&req.route.model).into(),
        usage: Usage {
            input_tokens: tokens(
                input,
                &if req.messages.is_empty() {
                    merged_prompt(req)
                } else {
                    serde_json::to_string(&req.messages).unwrap_or_default()
                },
            ),
            output_tokens: tokens(output, &text),
            cached_tokens: u["prompt_cache_hit_tokens"]
                .as_u64()
                .or_else(|| u["prompt_tokens_details"]["cached_tokens"].as_u64())
                .unwrap_or(0),
            cost_usd: u["cost"].as_f64().filter(|n| n.is_finite() && *n >= 0.0),
            metered: true,
            tokens_estimated: input.is_none() || output.is_none(),
        },
        text,
        tool_calls,
    })
}

pub async fn run_deepseek(_runner: &Runner, req: &RunRequest) -> Result<AdapterResult> {
    #[cfg(test)]
    let fixture = _runner.test_deepseek.clone();
    #[cfg(not(test))]
    let fixture: Option<(String, String)> = None;
    let (endpoint, key) = if let Some(fixture) = fixture {
        fixture
    } else {
        ("https://api.deepseek.com/chat/completions".into(), super::deepseek_key().ok_or_else(|| GenError::Api("DeepSeek API key is not configured. Add it in Settings or set DEEPSEEK_API_KEY.".into()))?)
    };
    let client = reqwest::Client::builder()
        .timeout(req.timeout)
        .build()
        .map_err(|e| GenError::Api(e.to_string()))?;
    let response = client
        .post(endpoint)
        .bearer_auth(key.trim())
        .json(&deepseek_body(req))
        .send()
        .await
        .map_err(|e| GenError::Api(format!("DeepSeek request failed: {}", e.without_url())))?;
    let status = response.status();
    if !status.is_success() {
        let reason = match status.as_u16() {
            401 | 403 => "key rejected; check API credentials",
            402 => "insufficient API credit",
            404 => "model or endpoint unavailable; check the selected model",
            429 => "rate limit reached; wait before retrying",
            _ => "provider request failed; try again later",
        };
        return Err(GenError::Api(format!("DeepSeek {status}: {reason}")));
    }
    let value = response
        .json::<Value>()
        .await
        .map_err(|_| GenError::Parse("DeepSeek returned an unreadable response".into()))?;
    parse_compatible(value, req)
}
