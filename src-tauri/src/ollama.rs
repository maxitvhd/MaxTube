use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize)]
struct GenerateResponse {
    response: Option<String>,
    thinking: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TagsResponse {
    #[serde(default)]
    models: Vec<ModelInfo>,
}

#[derive(Debug, Deserialize)]
struct ModelInfo {
    name: String,
}

fn client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .expect("http client")
}

/// Verifica se o servidor Ollama responde e se o modelo pedido esta instalado.
pub fn ensure_ready(base_url: &str, model: &str) -> Result<()> {
    let resp = client()
        .get(format!("{}/api/tags", base_url.trim_end_matches('/')))
        .send()
        .map_err(|_| {
            anyhow!(
                "Ollama nao respondeu em {}.\nAbra o app Ollama ou rode `ollama serve`.",
                base_url
            )
        })?;
    let tags: TagsResponse = resp.json().context("resposta invalida do Ollama")?;
    let base = model.split(':').next().unwrap_or(model);
    let found = tags
        .models
        .iter()
        .any(|m| m.name == model || m.name.starts_with(&format!("{}:", base)));
    if !found {
        bail!(
            "modelo '{}' nao encontrado no Ollama.\nBaixe com:  ollama pull {}",
            model,
            model
        );
    }
    Ok(())
}

/// Envia o prompt e retorna apenas o texto gerado (sem streaming).
pub fn generate(base_url: &str, model: &str, prompt: &str) -> Result<String> {
    let body = json!({
        "model": model,
        "prompt": prompt,
        "stream": false,
        "think": false,
        "options": { "temperature": 0.7 }
    });
    let resp = client()
        .post(format!("{}/api/generate", base_url.trim_end_matches('/')))
        .json(&body)
        .send()
        .context("falha ao chamar o Ollama")?;
    let parsed: GenerateResponse = resp.json().context("resposta invalida do Ollama")?;
    if let Some(err) = parsed.error {
        bail!("Ollama retornou erro: {}", err);
    }
    // Modelos com "thinking" as vezes deixam response vazio e o texto no thinking.
    let raw = match parsed.response {
        Some(r) if !r.trim().is_empty() => r,
        _ => parsed.thinking.unwrap_or_default(),
    };
    let text = clean_title(&raw);
    if text.is_empty() {
        bail!("Ollama nao retornou texto");
    }
    Ok(text)
}

/// Limpa a saida do modelo: remove aspas, prefixos e quebras de linha.
pub fn clean_title(raw: &str) -> String {
    let mut t = raw.trim().to_string();
    // pega a primeira linha nao vazia
    if let Some(line) = t.lines().map(|l| l.trim()).find(|l| !l.is_empty()) {
        t = line.to_string();
    }
    // remove prefixos comuns
    for prefix in [
        "Novo titulo:", "Novo título:", "Titulo:", "Título:", "TITLE:", "Title:", "-",
    ] {
        if let Some(rest) = t.strip_prefix(prefix) {
            t = rest.trim().to_string();
        }
    }
    // remove aspas envolventes
    let trimmed = t.trim_matches(|c| c == '"' || c == '\'' || c == '“' || c == '”').trim();
    trimmed.chars().take(100).collect()
}
