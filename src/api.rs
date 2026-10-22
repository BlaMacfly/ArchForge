//! API locale de l'interface et fichiers de l'interface.
//!
//! L'API exécute le SQL qu'on lui donne : elle n'écoute que sur 127.0.0.1 et exige, à chaque appel, la clé
//! tirée au démarrage et glissée dans la page (une autre page ouverte dans le navigateur ne peut pas la
//! lire), ainsi qu'un en-tête `Host` local (contre le rattachement DNS).

use crate::content::{ContentDir, ContentFile};
use crate::db::{Db, ResultSet, Settings};
use crate::{archcore, content};
use axum::extract::{Path as UrlPath, Request, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use rust_embed::RustEmbed;
use serde::Deserialize;
use serde_json::{json, Value};
use std::hash::{BuildHasher, Hasher};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(RustEmbed)]
#[folder = "web/"]
struct Assets;

pub struct AppState {
    archcore: Option<PathBuf>,
    db: RwLock<Option<Db>>,
    content: RwLock<Option<PathBuf>>,
    token: String,
}

impl AppState {
    pub fn new(archcore: Option<PathBuf>) -> Arc<AppState> {
        let content = archcore.as_deref().map(content::find_in);
        Arc::new(AppState { archcore, db: RwLock::new(None), content: RwLock::new(content), token: new_token() })
    }
}

/// Clé de 128 bits : deux tirages de l'aléa que la bibliothèque standard réserve aux tables de hachage.
fn new_token() -> String {
    let mut out = String::new();
    for i in 0..2u64 {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u64(i);
        h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos());
        out.push_str(&format!("{:016x}", h.finish()));
    }
    out
}

struct ApiError(anyhow::Error);

impl<E: Into<anyhow::Error>> From<E> for ApiError {
    fn from(e: E) -> Self {
        ApiError(e.into())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let message = match self.0.downcast_ref::<mysql_async::Error>() {
            Some(mysql_async::Error::Server(e)) => format!("MariaDB {} : {}", e.code, e.message),
            _ => format!("{:#}", self.0),
        };
        (StatusCode::BAD_REQUEST, Json(json!({ "error": message }))).into_response()
    }
}

type ApiResult<T> = Result<Json<T>, ApiError>;

pub fn router(state: Arc<AppState>) -> Router {
    let api = Router::new()
        .route("/defaults", get(defaults))
        .route("/status", get(status))
        .route("/connect", post(connect))
        .route("/disconnect", post(disconnect))
        .route("/overview", get(overview))
        .route("/select", post(select))
        .route("/execute", post(execute))
        .route("/content", get(content_list).post(content_append))
        .route("/content/dir", post(content_dir))
        .route("/content/file/{name}", get(content_read))
        .layer(middleware::from_fn_with_state(state.clone(), guard));
    Router::new()
        .nest("/api", api)
        .route("/", get(index))
        .route("/{*path}", get(asset))
        .with_state(state)
}

fn local_host(headers: &HeaderMap) -> bool {
    let Some(host) = headers.get(header::HOST).and_then(|h| h.to_str().ok()) else { return false };
    let name = host.rsplit_once(':').map_or(host, |(n, _)| n);
    matches!(name, "127.0.0.1" | "localhost" | "[::1]")
}

async fn guard(State(state): State<Arc<AppState>>, request: Request, next: Next) -> Response {
    let headers = request.headers();
    let token_ok = headers.get("x-archforge-token").and_then(|t| t.to_str().ok()) == Some(state.token.as_str());
    if !local_host(headers) || !token_ok {
        return (StatusCode::FORBIDDEN, Json(json!({ "error": "accès refusé : recharger la page d'ArchForge" }))).into_response();
    }
    next.run(request).await
}

async fn index(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    if !local_host(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let page = Assets::get("index.html").expect("web/index.html embarqué");
    let html = String::from_utf8_lossy(&page.data).replace("__ARCHFORGE_TOKEN__", &state.token);
    ([(header::CACHE_CONTROL, "no-store")], Html(html)).into_response()
}

async fn asset(UrlPath(path): UrlPath<String>) -> Response {
    match Assets::get(&path) {
        Some(file) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            ([(header::CONTENT_TYPE, mime.as_ref().to_string()), (header::CACHE_CONTROL, "no-cache".into())], file.data).into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

fn public_settings(s: &Settings) -> Value {
    let mut v = serde_json::to_value(s).unwrap_or_default();
    v["password"] = Value::String(String::new());
    v
}

async fn defaults(State(state): State<Arc<AppState>>) -> ApiResult<Value> {
    let settings = archcore::default_settings(state.archcore.as_deref());
    Ok(Json(json!({
        "archcore": state.archcore.as_ref().map(|p| p.display().to_string()),
        "settings": settings,
    })))
}

async fn status(State(state): State<Arc<AppState>>) -> ApiResult<Value> {
    let db = state.db.read().await;
    let content = state.content.read().await;
    Ok(Json(json!({
        "connected": db.is_some(),
        "version": db.as_ref().map(|d| d.version.clone()),
        "settings": db.as_ref().map(|d| public_settings(&d.settings)),
        "archcore": state.archcore.as_ref().map(|p| p.display().to_string()),
        "content": content.as_ref().map(|p| p.display().to_string()),
    })))
}

async fn connect(State(state): State<Arc<AppState>>, Json(settings): Json<Settings>) -> ApiResult<Value> {
    let db = Db::connect(settings).await?;
    let version = db.version.clone();
    if let Some(old) = state.db.write().await.replace(db) {
        old.disconnect().await;
    }
    Ok(Json(json!({ "version": version })))
}

async fn disconnect(State(state): State<Arc<AppState>>) -> ApiResult<Value> {
    if let Some(old) = state.db.write().await.take() {
        old.disconnect().await;
    }
    Ok(Json(json!({})))
}

macro_rules! connected {
    ($state:expr) => {{
        let guard = $state.db.read().await;
        if guard.is_none() {
            return Err(ApiError(anyhow::anyhow!("pas de connexion à MariaDB")));
        }
        guard
    }};
}

async fn overview(State(state): State<Arc<AppState>>) -> ApiResult<Value> {
    let db = connected!(state);
    Ok(Json(db.as_ref().unwrap().overview().await?))
}

#[derive(Deserialize)]
struct SelectBody {
    db: String,
    sql: String,
    #[serde(default)]
    params: Vec<Value>,
}

async fn select(State(state): State<Arc<AppState>>, Json(body): Json<SelectBody>) -> ApiResult<ResultSet> {
    let db = connected!(state);
    Ok(Json(db.as_ref().unwrap().select(&body.db, &body.sql, &body.params).await?))
}

#[derive(Deserialize)]
struct ExecuteBody {
    db: String,
    sql: String,
    #[serde(default = "yes")]
    transaction: bool,
}

fn yes() -> bool {
    true
}

async fn execute(State(state): State<Arc<AppState>>, Json(body): Json<ExecuteBody>) -> ApiResult<Vec<ResultSet>> {
    let db = connected!(state);
    Ok(Json(db.as_ref().unwrap().script(&body.db, &body.sql, body.transaction).await?))
}

async fn content_dir_or_err(state: &AppState) -> Result<ContentDir, ApiError> {
    match state.content.read().await.clone() {
        Some(path) => Ok(ContentDir { path }),
        None => Err(ApiError(anyhow::anyhow!("aucun dossier de contenu : lancer ArchForge avec --archcore <dépôt ArchCore>"))),
    }
}

async fn content_list(State(state): State<Arc<AppState>>) -> ApiResult<Vec<ContentFile>> {
    Ok(Json(content_dir_or_err(&state).await?.list()?))
}

async fn content_read(State(state): State<Arc<AppState>>, UrlPath(name): UrlPath<String>) -> ApiResult<Value> {
    let text = content_dir_or_err(&state).await?.read(&name)?;
    Ok(Json(json!({ "name": name, "text": text })))
}

#[derive(Deserialize)]
struct AppendBody {
    file: Option<String>,
    subject: Option<String>,
    title: String,
    sql: String,
}

async fn content_append(State(state): State<Arc<AppState>>, Json(body): Json<AppendBody>) -> ApiResult<Value> {
    let dir = content_dir_or_err(&state).await?;
    let name = dir.append(body.file.as_deref(), body.subject.as_deref(), &body.title, &body.sql)?;
    Ok(Json(json!({ "name": name, "path": dir.path.join(&name).display().to_string() })))
}

#[derive(Deserialize)]
struct DirBody {
    path: String,
}

async fn content_dir(State(state): State<Arc<AppState>>, Json(body): Json<DirBody>) -> ApiResult<Value> {
    let path = PathBuf::from(body.path.trim());
    if !path.is_dir() {
        return Err(ApiError(anyhow::anyhow!("{} n'est pas un dossier", path.display())));
    }
    *state.content.write().await = Some(path.clone());
    Ok(Json(json!({ "content": path.display().to_string() })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hotes_locaux_seulement() {
        let mut h = HeaderMap::new();
        h.insert(header::HOST, "127.0.0.1:8787".parse().unwrap());
        assert!(local_host(&h));
        h.insert(header::HOST, "localhost:8787".parse().unwrap());
        assert!(local_host(&h));
        h.insert(header::HOST, "evil.example:8787".parse().unwrap());
        assert!(!local_host(&h));
        assert!(!local_host(&HeaderMap::new()));
    }

    #[test]
    fn cles_differentes_a_chaque_lancement() {
        let (a, b) = (new_token(), new_token());
        assert_eq!(a.len(), 32);
        assert_ne!(a, b);
    }
}
