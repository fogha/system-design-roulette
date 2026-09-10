use super::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelOption {
    pub id: String,
    pub label: String,
    pub free: bool,
    pub input_usd_per_million: Option<f64>,
    pub output_usd_per_million: Option<f64>,
    pub context_length: Option<u64>,
    pub tools: bool,
    pub json_mode: bool,
    /// Provider-advertised reasoning controls, absent for non-reasoners/routers.
    #[serde(default)]
    pub reasoning: Option<Value>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCatalog {
    pub models: Vec<ModelOption>,
    pub source: String,
    pub error: Option<String>,
}
type Cache = HashMap<RunnerId, (Instant, ModelCatalog)>;
static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
pub fn invalidate(id: RunnerId) {
    if let Some(cache) = CACHE.get() {
        cache.lock().unwrap().remove(&id);
    }
}
pub fn free_model_id(id: &str) -> bool {
    id == "openrouter/free" || id.ends_with(":free")
}
pub fn option(id: &str) -> ModelOption {
    ModelOption {
        id: id.into(),
        label: id.into(),
        free: false,
        input_usd_per_million: None,
        output_usd_per_million: None,
        context_length: None,
        tools: false,
        json_mode: true,
        reasoning: None,
    }
}
fn price(value: &Value) -> Option<f64> {
    value
        .as_str()
        .and_then(|v| v.parse::<f64>().ok())
        .or_else(|| value.as_f64())
        .filter(|v| v.is_finite() && *v >= 0.0)
        .map(|v| v * 1_000_000.0)
}
pub fn decode(id: RunnerId, value: Value) -> Vec<ModelOption> {
    let rows = if id == RunnerId::GoogleApi || id == RunnerId::OllamaApi {
        &value["models"]
    } else {
        &value["data"]
    };
    let mut out: Vec<_> = rows
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|row| {
            let name = row["id"]
                .as_str()
                .or_else(|| row["name"].as_str())?
                .trim_start_matches("models/");
            if id == RunnerId::GoogleApi
                && !row["supportedGenerationMethods"]
                    .as_array()
                    .is_some_and(|a| a.iter().any(|v| v == "generateContent"))
            {
                return None;
            }
            if name.contains("embedding")
                || name.contains("embed-")
                || name.contains("whisper")
                || name.contains("tts")
                || name.contains("dall-e")
            {
                return None;
            }
            let mut model = option(name);
            model.label = row["displayName"]
                .as_str()
                .or_else(|| row["name"].as_str())
                .unwrap_or(name)
                .trim_start_matches("models/")
                .into();
            if id == RunnerId::OpenrouterApi {
                model.reasoning = row.get("reasoning").filter(|v| v.is_object()).cloned();
                model.input_usd_per_million = price(&row["pricing"]["prompt"]);
                model.output_usd_per_million = price(&row["pricing"]["completion"]);
                model.free = model.input_usd_per_million == Some(0.0)
                    && model.output_usd_per_million == Some(0.0);
                model.context_length = row["context_length"].as_u64();
                let params = row["supported_parameters"].as_array();
                model.tools = params.is_some_and(|p| p.iter().any(|v| v == "tools"));
                model.json_mode = params.is_some_and(|p| {
                    p.iter()
                        .any(|v| v == "response_format" || v == "structured_outputs")
                });
            }
            if id == RunnerId::OllamaApi {
                model.free = true;
            }
            Some(model)
        })
        .collect();
    out.sort_by(|a, b| b.free.cmp(&a.free).then(a.label.cmp(&b.label)));
    out.dedup_by(|a, b| a.id == b.id);
    out
}
pub async fn discover(_runner: &Runner, id: RunnerId, refresh: bool) -> Result<ModelCatalog> {
    if id.kind() == "cli" {
        return Ok(ModelCatalog {
            models: if id == RunnerId::ClaudeCli {
                ["default", "opus", "sonnet", "haiku"]
                    .into_iter()
                    .map(option)
                    .collect()
            } else {
                vec![option("default")]
            },
            source: "CLI aliases; enter a full model ID for another model".into(),
            error: None,
        });
    }
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if !refresh && id != RunnerId::OllamaApi {
        if let Some((at, result)) = cache.lock().unwrap().get(&id) {
            if at.elapsed() < Duration::from_secs(6 * 3600) {
                return Ok(result.clone());
            }
        }
    }
    let key = api::key(id);
    if id.metered() && id != RunnerId::OpenrouterApi && key.is_none() {
        return Ok(ModelCatalog {
            models: vec![],
            source: "API key required".into(),
            error: None,
        });
    }
    let url = if id == RunnerId::OllamaApi {
        format!("{}/api/tags", local::base()?)
    } else {
        format!("{}/models", api::base(id)?)
    };
    let request = api::authorize(
        reqwest::Client::new()
            .get(url)
            .timeout(Duration::from_secs(12)),
        id,
        key.as_deref(),
    );
    let found = async {
        let response = request
            .send()
            .await
            .map_err(|e| GenError::Api(e.without_url().to_string()))?;
        if !response.status().is_success() {
            return Err(GenError::Api(format!(
                "model discovery returned {}",
                response.status()
            )));
        }
        let value = response
            .json::<Value>()
            .await
            .map_err(|_| GenError::Parse("unreadable model catalogue".into()))?;
        Ok::<_, GenError>(decode(id, value))
    }
    .await;
    match found {
        Ok(mut models) => {
            if id == RunnerId::OpenrouterApi && !models.iter().any(|m| m.id == "openrouter/free") {
                let mut free = option("openrouter/free");
                free.label = "Free Models Router".into();
                free.free = true;
                models.insert(0, free);
            }
            let result = ModelCatalog {
                models,
                source: "Live provider catalogue".into(),
                error: None,
            };
            cache
                .lock()
                .unwrap()
                .insert(id, (Instant::now(), result.clone()));
            Ok(result)
        }
        Err(error) => {
            let mut result = cache
                .lock()
                .unwrap()
                .get(&id)
                .map(|(_, v)| v.clone())
                .unwrap_or(ModelCatalog {
                    models: vec![],
                    source: "Model catalogue unavailable; enter a model ID".into(),
                    error: None,
                });
            result.error = Some(error.to_string());
            Ok(result)
        }
    }
}
