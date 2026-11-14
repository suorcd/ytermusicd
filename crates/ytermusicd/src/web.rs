use axum::{
    http::{StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use rust_embed::{EmbeddedFile, RustEmbed};

#[derive(RustEmbed)]
#[folder = "$CARGO_MANIFEST_DIR/../ytermusicd-web/dist/"]
struct Assets;

/// Serve embedded web assets and use `index.html` for client-side routes.
pub async fn static_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');

    if uri.path() == "/api" || uri.path().starts_with("/api/") {
        return StatusCode::NOT_FOUND.into_response();
    }

    let key = if path.is_empty() { "index.html" } else { path };
    if let Some(asset) = Assets::get(key) {
        return asset_response(key, asset);
    }

    if path.contains('.') {
        return StatusCode::NOT_FOUND.into_response();
    }

    match Assets::get("index.html") {
        Some(asset) => asset_response("index.html", asset),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

fn asset_response(path: &str, asset: EmbeddedFile) -> Response {
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    let cache = if path.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    (
        [
            (header::CONTENT_TYPE, mime.as_ref().to_string()),
            (header::CACHE_CONTROL, cache.to_string()),
        ],
        asset.data.into_owned(),
    )
        .into_response()
}
