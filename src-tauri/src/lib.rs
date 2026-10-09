mod auth;
mod config;
mod ollama;
mod store;
mod youtube;

use config::Config;
use serde::Serialize;
use store::{Proposal, Status};
use tauri::State;
use youtube::Video;

struct AppState {
    cfg: Config,
}

#[derive(Serialize)]
struct StatusDto {
    logged_in: bool,
    has_credentials: bool,
    model: String,
    ollama_url: String,
    prompt: String,
    videos: usize,
    pending: usize,
    approved: usize,
    rejected: usize,
    applied: usize,
    uploads: usize,
    lives: usize,
}

#[derive(Serialize)]
struct ApplyResult {
    applied: usize,
    failed: usize,
}

#[tauri::command]
fn get_status(state: State<'_, AppState>) -> Result<StatusDto, String> {
    let cfg = &state.cfg;
    let videos = store::load_videos(cfg).unwrap_or_default();
    let proposals = store::load_proposals(cfg).unwrap_or_default();
    let (mut pending, mut approved, mut rejected, mut applied) = (0, 0, 0, 0);
    for p in &proposals {
        match p.status {
            Status::Pending => pending += 1,
            Status::Approved => approved += 1,
            Status::Rejected => rejected += 1,
            Status::Applied => applied += 1,
        }
    }
    let uploads = videos.iter().filter(|v| v.kind == "upload" || v.kind.is_empty()).count();
    let lives = videos.iter().filter(|v| v.kind == "live").count();
    Ok(StatusDto {
        logged_in: auth::load_token(&cfg.token).is_some(),
        has_credentials: auth::peek_client_id(&cfg.client_secret).is_some(),
        model: cfg.model.clone(),
        ollama_url: cfg.ollama_url.clone(),
        prompt: cfg.read_prompt().unwrap_or_default(),
        videos: videos.len(),
        pending,
        approved,
        rejected,
        applied,
        uploads,
        lives,
    })
}

#[tauri::command]
fn save_credentials(
    state: State<'_, AppState>,
    client_id: Option<String>,
    client_secret: Option<String>,
    json: Option<String>,
) -> Result<(), String> {
    let cfg = &state.cfg;
    if let Some(raw) = json.filter(|j| !j.trim().is_empty()) {
        return auth::write_client_secret_raw(&cfg.client_secret, &raw).map_err(|e| format!("{:#}", e));
    }
    let id = client_id.unwrap_or_default();
    let secret = client_secret.unwrap_or_default();
    if id.trim().is_empty() || secret.trim().is_empty() {
        return Err("Preencha o Client ID e o Client Secret (ou cole o JSON completo)".into());
    }
    auth::write_client_secret(&cfg.client_secret, id.trim(), secret.trim())
        .map_err(|e| format!("{:#}", e))
}

#[tauri::command]
async fn login(state: State<'_, AppState>) -> Result<(), String> {
    let cfg = state.cfg.clone();
    tauri::async_runtime::spawn_blocking(move || {
        auth::login(&cfg).map(|_| ()).map_err(|e| format!("{:#}", e))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn set_model(state: State<'_, AppState>, model: String) -> Result<(), String> {
    let mut cfg = state.cfg.clone();
    cfg.model = model;
    cfg.save().map_err(|e| format!("{:#}", e))
}

#[tauri::command]
fn set_ollama_url(state: State<'_, AppState>, url: String) -> Result<(), String> {
    let mut cfg = state.cfg.clone();
    cfg.ollama_url = url;
    cfg.save().map_err(|e| format!("{:#}", e))
}

#[tauri::command]
fn save_prompt(state: State<'_, AppState>, text: String) -> Result<(), String> {
    state.cfg.write_prompt(&text).map_err(|e| format!("{:#}", e))
}

#[tauri::command]
async fn pull(state: State<'_, AppState>, limit: Option<usize>) -> Result<usize, String> {
    let cfg = state.cfg.clone();
    tauri::async_runtime::spawn_blocking(move || -> anyhow::Result<usize> {
        let token = auth::access_token(&cfg)?;
        let playlist = youtube::uploads_playlist(&token)?;
        let ids = youtube::list_video_ids(&token, &playlist, limit)?;
        let videos = youtube::get_videos(&token, &ids)?;
        store::save_videos(&cfg, &videos)?;
        Ok(videos.len())
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{:#}", e))
}

#[tauri::command]
fn list_videos(state: State<'_, AppState>) -> Result<Vec<Video>, String> {
    store::load_videos(&state.cfg).map_err(|e| format!("{:#}", e))
}

#[tauri::command]
fn list_proposals(state: State<'_, AppState>) -> Result<Vec<Proposal>, String> {
    store::load_proposals(&state.cfg).map_err(|e| format!("{:#}", e))
}

#[tauri::command]
async fn generate(
    state: State<'_, AppState>,
    model: Option<String>,
    redo: bool,
) -> Result<Vec<Proposal>, String> {
    let cfg = state.cfg.clone();
    tauri::async_runtime::spawn_blocking(move || -> anyhow::Result<Vec<Proposal>> {
        let model = model.filter(|m| !m.trim().is_empty()).unwrap_or_else(|| cfg.model.clone());
        let videos = store::load_videos(&cfg)?;
        if videos.is_empty() {
            anyhow::bail!("Nenhum video salvo. Clique em \"Puxar videos\" primeiro.");
        }
        ollama::ensure_ready(&cfg.ollama_url, &model)?;
        let template = cfg.read_prompt()?;

        let mut proposals = if redo { Vec::new() } else { store::load_proposals(&cfg)? };
        // pula videos que ja tem sugestao (sem "refazer" nao gera de novo nem perde revisao)
        let existing: std::collections::HashSet<String> =
            proposals.iter().map(|p| p.video_id.clone()).collect();
        // conta as falhas do Ollama para avisar no final em vez de esconder
        let mut falhas = 0usize;
        let mut ultimo_erro = String::new();

        for v in &videos {
            if existing.contains(&v.id) {
                continue;
            }
            let prompt = template
                .replace("{{title}}", &v.title)
                .replace("{{description}}", &v.description);
            match ollama::generate(&cfg.ollama_url, &model, &prompt) {
                Ok(title) => {
                    if title.trim().is_empty() {
                        falhas += 1;
                        ultimo_erro = "Ollama devolveu titulo vazio".into();
                        continue;
                    }
                    proposals.push(Proposal {
                        video_id: v.id.clone(),
                        original_title: v.title.clone(),
                        proposed_title: title,
                        proposed_description: String::new(),
                        status: Status::Pending,
                    });
                    let _ = store::save_proposals(&cfg, &proposals);
                }
                Err(e) => {
                    falhas += 1;
                    ultimo_erro = format!("{:#}", e);
                }
            }
        }
        store::save_proposals(&cfg, &proposals)?;
        if falhas > 0 {
            anyhow::bail!(
                "{} sugestoes geradas, {} falharam no Ollama. Ultimo erro: {}",
                proposals.len(),
                falhas,
                ultimo_erro
            );
        }
        Ok(proposals)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{:#}", e))
}

#[tauri::command]
fn edit_proposal(
    state: State<'_, AppState>,
    video_id: String,
    title: String,
    description: Option<String>,
) -> Result<(), String> {
    let cfg = &state.cfg;
    let mut proposals = store::load_proposals(cfg).map_err(|e| format!("{:#}", e))?;
    if let Some(p) = proposals.iter_mut().find(|p| p.video_id == video_id) {
        p.proposed_title = title.chars().take(100).collect();
        if let Some(d) = description {
            p.proposed_description = d.clone();
        }
    }
    store::save_proposals(cfg, &proposals).map_err(|e| format!("{:#}", e))
}

#[tauri::command]
fn set_status(state: State<'_, AppState>, video_id: String, status: String) -> Result<(), String> {
    let cfg = &state.cfg;
    let new = match status.as_str() {
        "pending" => Status::Pending,
        "approved" => Status::Approved,
        "rejected" => Status::Rejected,
        "applied" => Status::Applied,
        other => return Err(format!("status invalido: {}", other)),
    };
    let mut proposals = store::load_proposals(cfg).map_err(|e| format!("{:#}", e))?;
    if let Some(p) = proposals.iter_mut().find(|p| p.video_id == video_id) {
        p.status = new;
    }
    store::save_proposals(cfg, &proposals).map_err(|e| format!("{:#}", e))
}

#[tauri::command]
async fn apply(state: State<'_, AppState>) -> Result<ApplyResult, String> {
    let cfg = state.cfg.clone();
    tauri::async_runtime::spawn_blocking(move || -> anyhow::Result<ApplyResult> {
        let mut proposals = store::load_proposals(&cfg)?;
        let mut videos = store::load_videos(&cfg)?;
        let token = auth::access_token(&cfg)?;

        let targets: Vec<usize> = proposals
            .iter()
            .enumerate()
            .filter(|(_, p)| p.status == Status::Approved)
            .map(|(i, _)| i)
            .collect();

        let (mut applied, mut failed) = (0usize, 0usize);
        for i in targets {
            let p = proposals[i].clone();
            if let Some(pos) = videos.iter().position(|v| v.id == p.video_id) {
                match youtube::update_title(&token, &videos[pos], &p.proposed_title) {
                    Ok(_) => {
                        proposals[i].status = Status::Applied;
                        videos[pos].title = p.proposed_title.clone();
                        applied += 1;
                    }
                    Err(_) => failed += 1,
                }
            } else {
                failed += 1;
            }
            let _ = store::save_proposals(&cfg, &proposals);
            let _ = store::save_videos(&cfg, &videos);
        }
        Ok(ApplyResult { applied, failed })
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{:#}", e))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let cfg = Config::load().expect("falha ao carregar a configuracao");
    let _ = cfg.ensure_dirs();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState { cfg })
        .invoke_handler(tauri::generate_handler![
            get_status,
            save_credentials,
            login,
            set_model,
            set_ollama_url,
            save_prompt,
            pull,
            list_videos,
            list_proposals,
            generate,
            edit_proposal,
            set_status,
            apply
        ])
        .run(tauri::generate_context!())
        .expect("erro ao executar o app");
}
