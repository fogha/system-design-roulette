use super::*;
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    sync::{Arc, Mutex},
};

fn fixture() -> Runner {
    let root = std::env::temp_dir().join(new_call_id());
    std::fs::create_dir_all(&root).unwrap();
    let database = root.join("fixture.db");
    crate::db::open(&database).unwrap();
    Runner {
        claude_bin: "/definitely/missing/claude".into(),
        codex_bin: Some("none".into()),
        scratch_dir: root.join("scratch"),
        database: Some(database),
        log_tx: None,
        test_deepseek: None,
    }
}
fn request(id: RunnerId, prompt: &str) -> RunRequest {
    RunRequest::new(
        Route {
            runner: id,
            model: "fixture-model".into(),
            custom_command: String::new(),
        },
        prompt,
    )
}

#[test]
fn runner_routes_cover_cli_api_and_local_without_colliding_provider_ids() {
    let mut ids = std::collections::HashSet::new();
    for id in RunnerId::ALL {
        assert!(ids.insert(id.id()));
        assert_eq!(RunnerId::parse(id.id()), Some(id));
        assert_eq!(RunnerId::parse(id.legacy_id()), Some(id));
        assert_eq!(id.key_name().is_some(), id.kind() == "api");
        assert!(valid_model(&default_model(id)));
    }
    assert_eq!(
        RunnerId::ALL.iter().filter(|r| r.kind() == "cli").count(),
        5
    );
    assert_eq!(
        RunnerId::ALL.iter().filter(|r| r.kind() == "api").count(),
        7
    );
    assert_eq!(RunnerId::OllamaApi.kind(), "local");
    assert!(!RunnerId::OllamaApi.metered());
}

#[tokio::test]
async fn hosted_and_local_adapters_send_selected_model_and_decode_provider_answers() {
    for id in [
        RunnerId::AnthropicApi,
        RunnerId::OpenaiApi,
        RunnerId::GoogleApi,
        RunnerId::OpenrouterApi,
        RunnerId::GroqApi,
        RunnerId::MistralApi,
        RunnerId::OllamaApi,
    ] {
        let reply = match id {
            RunnerId::AnthropicApi => {
                json!({"content":[{"type":"thinking","thinking":"private"},{"type":"text","text":"{\"status\":\"pong\"}"}],"usage":{"input_tokens":10,"cache_read_input_tokens":3,"output_tokens":5}})
            }
            RunnerId::GoogleApi => {
                json!({"candidates":[{"content":{"parts":[{"thought":true,"text":"private"},{"text":"{\"status\":\"pong\"}"}]}}],"usageMetadata":{"promptTokenCount":10,"candidatesTokenCount":5}})
            }
            _ => {
                json!({"choices":[{"message":{"content":"{\"status\":\"pong\"}"}}],"usage":{"prompt_tokens":10,"completion_tokens":5}})
            }
        };
        let (endpoint, server) = server(vec![reply]);
        let mut req = request(id, "Return JSON with status pong.");
        req.system = Some("Teach using course evidence.".into());
        let result = api::send(
            &req,
            endpoint.trim_end_matches("/chat/completions"),
            Some("fixture-not-a-secret"),
            None,
        )
        .await
        .unwrap();
        assert_eq!(parse_json(&result.text).unwrap()["status"], "pong");
        assert!(!result.text.contains("private"));
        assert_eq!(result.model, "fixture-model");
        let wire = server.join().unwrap().remove(0);
        match id {
            RunnerId::AnthropicApi => {
                assert_eq!(wire["system"], "Teach using course evidence.");
                assert_eq!(wire["model"], "fixture-model");
                assert!(wire.get("response_format").is_none());
            }
            RunnerId::GoogleApi => {
                assert_eq!(
                    wire["systemInstruction"]["parts"][0]["text"],
                    "Teach using course evidence."
                );
                assert_eq!(
                    wire["generationConfig"]["responseMimeType"],
                    "application/json"
                );
            }
            _ => {
                assert_eq!(wire["model"], "fixture-model");
                assert_eq!(
                    wire["messages"][0]["content"],
                    "Teach using course evidence."
                );
                assert_eq!(wire["response_format"]["type"], "json_object");
            }
        }
        if id == RunnerId::OpenaiApi {
            assert_eq!(wire["store"], false);
            assert!(wire.get("max_tokens").is_none());
            assert!(wire["max_completion_tokens"].is_number());
        }
        assert!(wire.get("thinking").is_none());
        assert_eq!(result.usage.metered, id != RunnerId::OllamaApi);
        assert_eq!(
            result.usage.cost_usd,
            if id == RunnerId::OllamaApi {
                Some(0.0)
            } else {
                None
            }
        );
    }
}

#[test]
fn provider_urls_and_credentials_follow_each_protocol() {
    for (id, path, header) in [
        (RunnerId::AnthropicApi, "/v1/messages", "x-api-key"),
        (
            RunnerId::GoogleApi,
            "/v1/models/fixture-model:generateContent",
            "x-goog-api-key",
        ),
        (RunnerId::OpenaiApi, "/v1/chat/completions", "authorization"),
    ] {
        let url = api::request_url(id, "https://example.test/v1", "fixture-model").unwrap();
        assert_eq!(url.path(), path);
        let built = api::authorize(reqwest::Client::new().post(url), id, Some("fixture"))
            .build()
            .unwrap();
        assert!(built.headers().contains_key(header));
        assert!(built.url().query().is_none());
        if id != RunnerId::OpenaiApi {
            assert!(!built.headers().contains_key("authorization"));
        }
    }
    let url = api::request_url(
        RunnerId::OpenrouterApi,
        "https://example.test/v1",
        "vendor/model",
    )
    .unwrap();
    assert_eq!(url.path(), "/v1/chat/completions");
}

#[test]
fn model_catalogues_keep_real_prices_and_exclude_non_chat_models() {
    let models = models::decode(
        RunnerId::OpenrouterApi,
        json!({"data":[
            {"id":"vendor/paid","name":"Paid","pricing":{"prompt":"0.000001","completion":"0.000004"},"supported_parameters":["tools"]},
            {"id":"vendor/free:free","name":"Free","pricing":{"prompt":"0","completion":"0"},"supported_parameters":["response_format"]},
            {"id":"vendor/unknown","name":"Unknown","pricing":{"prompt":"invalid"}},
            {"id":"text-embedding-3-small"}
        ]}),
    );
    assert_eq!(models.len(), 3);
    assert_eq!(models[0].id, "vendor/free:free");
    assert!(models[0].free && models[0].json_mode);
    let paid = models.iter().find(|m| m.id == "vendor/paid").unwrap();
    assert_eq!(paid.input_usd_per_million, Some(1.0));
    assert_eq!(paid.output_usd_per_million, Some(4.0));
    assert!(paid.tools && !paid.json_mode);
    assert!(
        !models
            .iter()
            .find(|m| m.id == "vendor/unknown")
            .unwrap()
            .free
    );
    let google = models::decode(
        RunnerId::GoogleApi,
        json!({"models":[{"name":"models/text","supportedGenerationMethods":["generateContent"]},{"name":"models/embed","supportedGenerationMethods":["embedContent"]}]}),
    );
    assert_eq!(google.len(), 1);
    assert_eq!(google[0].id, "text");
    let mut req = request(RunnerId::OpenrouterApi, "Return JSON");
    req.json = true;
    assert!(api::body(&req, Some(paid)).get("response_format").is_none());
}

#[test]
fn local_tutor_rejects_cloud_and_embedding_weights_and_decodes_download_termination() {
    for name in ["", "foo;touch /tmp/no", "model:cloud", "model-cloud"] {
        assert!(local::validate_name(name).is_err());
    }
    assert!(local::validate_name("community/model:7b-q4_K_M").is_ok());
    assert!(local::validate_chat_capabilities(json!({"capabilities":["embedding"]})).is_err());
    assert!(local::validate_chat_capabilities(
        json!({"capabilities":["completion"],"remote_host":"https://ollama.com"})
    )
    .is_err());
    assert!(
        local::validate_chat_capabilities(json!({"capabilities":["completion","tools"]})).is_ok()
    );
    let mut state = local::PullState {
        model: "fixture".into(),
        status: String::new(),
        percent: None,
        done: false,
        error: None,
    };
    assert!(!local::decode_progress(
        br#"{"status":"pulling layer","total":100,"completed":57}"#,
        &mut state
    )
    .unwrap());
    assert_eq!(state.percent, Some(57));
    local::decode_progress(br#"{"total":100,"completed":105}"#, &mut state).unwrap();
    assert_eq!(state.percent, Some(100));
    // Final NDJSON record is valid with no trailing newline.
    assert!(local::decode_progress(br#"{"status":"success"}"#, &mut state).unwrap());
    assert!(local::decode_progress(br#"{"error":"disk full"}"#, &mut state).is_err());
    assert!(local::decode_progress(b"truncated", &mut state).is_err());
}

#[test]
fn json_recovery_handles_arrays_nested_fences_and_trailing_braces() {
    let text = "Here: ```json\n{\"markdown\":\"```sh\\necho \\\"}\\\"\\n```\",\"nested\":[{\"ok\":true}]}\n``` explanation {different}";
    assert_eq!(parse_json(text).unwrap()["nested"][0]["ok"], true);
    assert_eq!(
        parse_json("Answer: [1,{\"brace\":\"]\"}] then {notes}").unwrap(),
        json!([1,{"brace":"]"}])
    );
    assert!(parse_json("{\"unfinished\":").is_none());
    assert_eq!(
        process::command_words("'/a path/tool' --model \"local model\" '{prompt}' '$(literal)'")
            .unwrap(),
        vec![
            "/a path/tool",
            "--model",
            "local model",
            "{prompt}",
            "$(literal)"
        ]
    );
    assert!(process::command_words("'unclosed").is_err());
    assert_eq!(effective_model(RunnerId::CodexCli, "opus"), "default");
    assert_eq!(
        effective_model(RunnerId::CodexCli, "my-codex-model"),
        "my-codex-model"
    );
    assert!(RunnerId::parse("unknown").is_none());
}

#[cfg(unix)]
fn script(runner: &Runner, name: &str, body: &str) -> String {
    use std::os::unix::fs::PermissionsExt;
    let path = runner
        .database
        .as_ref()
        .unwrap()
        .parent()
        .unwrap()
        .join(name);
    std::fs::write(&path, format!("#!/usr/bin/env python3\n{body}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    path.to_str().unwrap().into()
}

#[cfg(unix)]
#[tokio::test]
async fn concurrent_codex_calls_have_distinct_output_models_and_persisted_usage() {
    let mut runner = fixture();
    runner.codex_bin = Some(script(
        &runner,
        "codex fixture.py",
        r#"
import json, pathlib, sys, time
args=sys.argv
prompt=sys.stdin.read()
model=args[args.index('--model')+1]
time.sleep(0.04)
pathlib.Path(args[args.index('--output-last-message')+1]).write_text(json.dumps({'prompt':prompt,'model':model}))
print(json.dumps({'type':'turn.completed','usage':{'input_tokens':20,'cached_input_tokens':4,'output_tokens':7}}))
"#,
    ));
    let a = request(RunnerId::CodexCli, "first learner");
    let mut b = request(RunnerId::CodexCli, "second learner");
    b.route.model = "another-model".into();
    let (a, b) = tokio::join!(runner.run(&a, None), runner.run(&b, None));
    let a = a.unwrap();
    let b = b.unwrap();
    assert_ne!(a.call_id, b.call_id);
    assert_eq!(a.json.unwrap()["prompt"], "first learner");
    assert_eq!(b.json.unwrap()["model"], "another-model");
    assert_eq!(a.usage.input_tokens, 20);
    assert_eq!(a.usage.cached_tokens, 4);
    assert!(!a.usage.tokens_estimated);
    assert_eq!(std::fs::read_dir(&runner.scratch_dir).unwrap().count(), 0);
    let records = store::recent(runner.database.as_ref().unwrap()).unwrap();
    assert_eq!(records.len(), 2);
    assert!(records.iter().all(|r| r.status == "succeeded"));
}

#[cfg(unix)]
#[tokio::test]
async fn capture_drains_stderr_and_kills_process_group_on_timeout_and_abort() {
    let runner = fixture();
    let noisy = script(
        &runner,
        "noisy.py",
        "import sys\nsys.stderr.write('x'*1000000)\nsys.stderr.flush()\nprint('finished')",
    );
    assert_eq!(
        process::capture(
            tokio::process::Command::new(noisy),
            None,
            Duration::from_secs(3)
        )
        .await
        .unwrap()
        .trim(),
        "finished"
    );
    for cancel in [false, true] {
        let marker = runner.database.as_ref().unwrap().with_extension(if cancel {
            "abort"
        } else {
            "timeout"
        });
        let body=format!("import subprocess, sys, time\nsubprocess.Popen([sys.executable,'-c',{}])\ntime.sleep(20)",json!(format!("import pathlib,time;time.sleep(0.7);pathlib.Path({}).write_text('orphan')",json!(marker.to_str().unwrap()))));
        let slow = script(&runner, if cancel { "abort.py" } else { "slow.py" }, &body);
        if cancel {
            let task = tokio::spawn(async move {
                process::capture(
                    tokio::process::Command::new(slow),
                    None,
                    Duration::from_secs(4),
                )
                .await
            });
            tokio::time::sleep(Duration::from_millis(250)).await;
            task.abort();
            let _ = task.await;
        } else {
            assert!(matches!(
                process::capture(
                    tokio::process::Command::new(slow),
                    None,
                    Duration::from_millis(250)
                )
                .await,
                Err(GenError::Timeout(_))
            ));
        }
        tokio::time::sleep(Duration::from_millis(800)).await;
        assert!(!marker.exists(), "a descendant survived cancellation");
    }
}

#[cfg(unix)]
#[tokio::test]
async fn fallback_is_explicit_and_uses_its_own_model_and_health_requires_real_answer() {
    let mut runner = fixture();
    runner.codex_bin = Some("/usr/bin/false".into());
    runner.claude_bin = script(
        &runner,
        "claude.py",
        r#"
import json,sys
sys.stdin.read()
print(json.dumps({'type':'result','result':json.dumps({'status':'pong','model':sys.argv[sys.argv.index('--model')+1]}),'usage':{'input_tokens':2,'output_tokens':3},'total_cost_usd':0.00001}))
"#,
    );
    let req = request(RunnerId::CodexCli, "check");
    assert!(runner.run(&req, None).await.is_err());
    assert_eq!(
        store::recent(runner.database.as_ref().unwrap())
            .unwrap()
            .len(),
        1
    );
    let fallback = Route {
        runner: RunnerId::ClaudeCli,
        model: "fallback-model".into(),
        custom_command: String::new(),
    };
    let result = runner.run(&req, Some(&fallback)).await.unwrap();
    assert_eq!(result.runner, RunnerId::ClaudeCli);
    assert_eq!(result.json.unwrap()["model"], "fallback-model");
    let calls = store::recent(runner.database.as_ref().unwrap()).unwrap();
    assert!(calls[0].fallback_of.is_some());
    assert_eq!(calls.len(), 3);
    assert!(runner.check(fallback).await.ok);
    let bad = script(&runner, "garbage.py", "print('not authenticated')");
    assert!(
        !runner
            .check(Route {
                runner: RunnerId::CustomCli,
                model: "default".into(),
                custom_command: format!("'{bad}'")
            })
            .await
            .ok
    );
}

fn server(replies: Vec<Value>) -> (String, std::thread::JoinHandle<Vec<Value>>) {
    let socket = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/chat/completions", socket.local_addr().unwrap());
    let thread = std::thread::spawn(move || {
        let mut requests = vec![];
        for reply in replies {
            let (mut stream, _) = socket.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut bytes = vec![];
            let mut buf = [0; 4096];
            let body_start = loop {
                let n = stream.read(&mut buf).unwrap();
                assert_ne!(n, 0);
                bytes.extend_from_slice(&buf[..n]);
                if let Some(index) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    break index + 4;
                }
            };
            let header = String::from_utf8_lossy(&bytes[..body_start]);
            let len: usize = header
                .lines()
                .find_map(|l| {
                    l.to_lowercase()
                        .strip_prefix("content-length:")
                        .map(|v| v.trim().parse().unwrap())
                })
                .unwrap();
            while bytes.len() < body_start + len {
                let n = stream.read(&mut buf).unwrap();
                assert_ne!(n, 0);
                bytes.extend_from_slice(&buf[..n]);
            }
            requests.push(serde_json::from_slice(&bytes[body_start..body_start + len]).unwrap());
            let body = reply.to_string();
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
        }
        requests
    });
    (url, thread)
}

#[tokio::test]
async fn tool_loop_passes_actual_results_and_accounts_each_bounded_turn() {
    let mut runner = fixture();
    let (endpoint, server) = server(vec![
        json!({"choices":[{"message":{"tool_calls":[{"id":"fetch-1","function":{"name":"fetch_source","arguments":"{\"url\":\"https://example.test/source\"}"}}]}}],"usage":{"prompt_tokens":10,"completion_tokens":5,"cost":0.01}}),
        json!({"choices":[{"message":{"content":"{\"answer\":\"grounded\"}"}}],"usage":{"prompt_tokens":20,"completion_tokens":5,"cost":0.02}}),
    ]);
    runner.test_deepseek = Some((endpoint, "fixture-not-a-secret".into()));
    let mut req = request(RunnerId::DeepseekApi, "Use retrieved evidence.");
    req.tools = vec![ToolDef {
        name: "fetch_source".into(),
        description: "Fetch a course source".into(),
        parameters: json!({"type":"object"}),
    }];
    let executed = Arc::new(Mutex::new(vec![]));
    let observed = executed.clone();
    let result = runner
        .run_with_tools(&req, 2, move |call| {
            observed.lock().unwrap().push(call.name);
            async { Ok("actual source material".into()) }
        })
        .await
        .unwrap();
    assert_eq!(result.json.unwrap()["answer"], "grounded");
    assert_eq!(*executed.lock().unwrap(), vec!["fetch_source"]);
    let requests = server.join().unwrap();
    assert!(requests[0].get("response_format").is_none());
    assert!(requests[1].get("tools").is_none());
    assert_eq!(
        requests[1]["messages"][2]["content"],
        "actual source material"
    );
    assert_eq!(requests[1]["messages"][2]["tool_call_id"], "fetch-1");
    let calls = store::recent(runner.database.as_ref().unwrap()).unwrap();
    assert_eq!(calls.len(), 2);
    assert!((calls.iter().filter_map(|c| c.cost_usd).sum::<f64>() - 0.03).abs() < 1e-9);
}

#[tokio::test]
async fn final_tool_turn_cannot_execute_more_work_and_unknown_cost_is_not_free() {
    let mut runner = fixture();
    let (endpoint, server) = server(vec![
        json!({"choices":[{"message":{"tool_calls":[{"id":"no","function":{"name":"unexpected","arguments":"broken"}}]}}]}),
    ]);
    runner.test_deepseek = Some((endpoint, "fixture".into()));
    let req = request(RunnerId::DeepseekApi, "final");
    assert!(runner
        .run_with_tools(&req, 1, |_| async { panic!("final turn executed a tool") })
        .await
        .is_err());
    server.join().unwrap();
    let calls = store::recent(runner.database.as_ref().unwrap()).unwrap();
    assert!(calls[0].cost_usd.is_none());
    assert!(calls[0].tokens_estimated);
    let conn = crate::db::open(runner.database.as_ref().unwrap()).unwrap();
    crate::db::set_config(&conn, "agent_budget_monthly_usd", "1").unwrap();
    assert!(store::begin(
        runner.database.as_ref().unwrap(),
        &new_call_id(),
        &req,
        None
    )
    .unwrap_err()
    .to_string()
    .contains("cost is unknown"));
    let cli = request(RunnerId::CodexCli, "subscription");
    assert!(store::begin(
        runner.database.as_ref().unwrap(),
        &new_call_id(),
        &cli,
        None
    )
    .is_ok());
}

#[cfg(unix)]
#[tokio::test]
async fn persistence_failure_does_not_repeat_an_already_completed_provider_call() {
    let mut runner = fixture();
    runner.claude_bin=script(&runner,"claude.py","import sys,json\nsys.stdin.read()\nprint(json.dumps({'type':'result','result':'{\"ok\":true}'}))");
    let conn = crate::db::open(runner.database.as_ref().unwrap()).unwrap();
    conn.execute_batch("CREATE TRIGGER break_agent_save BEFORE UPDATE ON agent_calls BEGIN SELECT RAISE(ABORT, 'injected disk failure'); END;").unwrap();
    let fallback = Route {
        runner: RunnerId::CustomCli,
        model: "default".into(),
        custom_command: "/usr/bin/false".into(),
    };
    let error = runner
        .run(&request(RunnerId::ClaudeCli, "once"), Some(&fallback))
        .await
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("agent activity could not be saved"));
    let calls = store::recent(runner.database.as_ref().unwrap()).unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].status, "running");
}
