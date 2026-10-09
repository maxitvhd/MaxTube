#![allow(non_snake_case)]
use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;

const API: &str = "https://www.googleapis.com/youtube/v3";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Video {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub category_id: Option<String>,
    #[serde(default)]
    pub published_at: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct ChannelsResponse {
    #[serde(default)]
    items: Vec<ChannelItem>,
}

#[derive(Debug, Deserialize)]
struct ChannelItem {
    contentDetails: ChannelContentDetails,
}

#[derive(Debug, Deserialize)]
struct ChannelContentDetails {
    #[serde(rename = "relatedPlaylists")]
    related_playlists: RelatedPlaylists,
}

#[derive(Debug, Deserialize)]
struct RelatedPlaylists {
    uploads: String,
}

#[derive(Debug, Deserialize)]
struct PlaylistItemsResponse {
    #[serde(default)]
    nextPageToken: Option<String>,
    #[serde(default)]
    items: Vec<PlaylistItem>,
}

#[derive(Debug, Deserialize)]
struct PlaylistItem {
    contentDetails: PlaylistContentDetails,
}

#[derive(Debug, Deserialize)]
struct PlaylistContentDetails {
    videoId: String,
}

#[derive(Debug, Deserialize)]
struct VideosResponse {
    #[serde(default)]
    items: Vec<VideoResource>,
}

#[derive(Debug, Deserialize)]
struct VideoResource {
    id: String,
    snippet: VideoSnippet,
}

#[derive(Debug, Deserialize)]
struct VideoSnippet {
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    categoryId: Option<String>,
    #[serde(default)]
    publishedAt: Option<String>,
    #[serde(default)]
    tags: Option<Vec<String>>,
}

fn client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .expect("http client")
}

fn check(resp: reqwest::blocking::Response) -> Result<serde_json::Value> {
    let status = resp.status();
    let text = resp.text().unwrap_or_default();
    if !status.is_success() {
        bail!("YouTube API erro {}: {}", status, text);
    }
    Ok(serde_json::from_str(&text).context("resposta JSON invalida do YouTube")?)
}

/// Retorna a playlist de uploads do canal autenticado.
pub fn uploads_playlist(token: &str) -> Result<String> {
    let resp = client()
        .get(format!("{}/channels", API))
        .bearer_auth(token)
        .query(&[("part", "contentDetails"), ("mine", "true")])
        .send()
        .context("falha ao consultar o canal")?;
    let v: ChannelsResponse = serde_json::from_value(check(resp)?.into())
        .unwrap_or(ChannelsResponse { items: vec![] });
    let ch = v
        .items
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("nenhum canal encontrado para esta conta"))?;
    Ok(ch.contentDetails.related_playlists.uploads)
}

/// Lista os IDs dos videos da playlist de uploads (mais recentes primeiro).
pub fn list_video_ids(token: &str, playlist: &str, limit: Option<usize>) -> Result<Vec<String>> {
    let mut ids = Vec::new();
    let mut page: Option<String> = None;
    loop {
        let mut req = client()
            .get(format!("{}/playlistItems", API))
            .bearer_auth(token)
            .query(&[
                ("part", "contentDetails"),
                ("playlistId", playlist),
                ("maxResults", "50"),
            ]);
        if let Some(p) = &page {
            req = req.query(&[("pageToken", p.as_str())]);
        }
        let resp = req.send().context("falha ao listar playlist")?;
        let v: PlaylistItemsResponse =
            serde_json::from_value(check(resp)?.into()).unwrap_or(PlaylistItemsResponse {
                nextPageToken: None,
                items: vec![],
            });
        for item in v.items {
            ids.push(item.contentDetails.videoId);
        }
        if let Some(l) = limit {
            if ids.len() >= l {
                ids.truncate(l);
                break;
            }
        }
        match v.nextPageToken {
            Some(p) if !p.is_empty() => page = Some(p),
            _ => break,
        }
    }
    Ok(ids)
}

/// Busca os detalhes (snippet) dos videos por ID, em lotes de 50.
pub fn get_videos(token: &str, ids: &[String]) -> Result<Vec<Video>> {
    let mut out = Vec::new();
    for chunk in ids.chunks(50) {
        let joined = chunk.join(",");
        let resp = client()
            .get(format!("{}/videos", API))
            .bearer_auth(token)
            .query(&[("part", "snippet"), ("id", joined.as_str())])
            .send()
            .context("falha ao buscar detalhes dos videos")?;
        let v: VideosResponse = serde_json::from_value(check(resp)?.into())
            .unwrap_or(VideosResponse { items: vec![] });
        for it in v.items {
            out.push(Video {
                id: it.id,
                title: it.snippet.title,
                description: it.snippet.description,
                category_id: it.snippet.categoryId,
                published_at: it.snippet.publishedAt,
                tags: it.snippet.tags,
            });
        }
    }
    Ok(out)
}

/// Atualiza o titulo de um video.
pub fn update_title(token: &str, video: &Video, new_title: &str) -> Result<()> {
    let mut snippet = json!({ "title": new_title });
    if let Some(cat) = &video.category_id {
        snippet["categoryId"] = json!(cat);
    }
    if !video.description.is_empty() {
        snippet["description"] = json!(video.description);
    }
    let body = json!({ "id": video.id, "snippet": snippet });

    let resp = client()
        .put(format!("{}/videos", API))
        .bearer_auth(token)
        .query(&[("part", "snippet")])
        .json(&body)
        .send()
        .context("falha ao atualizar o titulo")?;
    check(resp)?;
    Ok(())
}
