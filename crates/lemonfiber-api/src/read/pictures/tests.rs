use axum::http::{header, StatusCode};
use lemonfiber_core::screening::Pictured;

use super::shown;

#[test]
fn a_picture_is_shown_as_its_raster_type_and_kept_by_its_asker_alone() {
    let response = shown(Pictured {
        media_type: "image/webp",
        bytes: b"webp".to_vec(),
    });
    let header = |name| {
        response
            .headers()
            .get(name)
            .and_then(|value| value.to_str().ok())
    };
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(header(header::CONTENT_TYPE), Some("image/webp"));
    assert_eq!(header(header::CACHE_CONTROL), Some("private"));
    assert_eq!(header(header::X_CONTENT_TYPE_OPTIONS), Some("nosniff"));
}
