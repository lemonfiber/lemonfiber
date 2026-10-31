use super::{image, raster};

const PNG: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13];

const JPEG: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, 0, 16];

const WEBP: &[u8] = b"RIFF\x24\x00\x00\x00WEBPVP8 ";

const AVIF: &[u8] = b"\x00\x00\x00\x1cftypavif\x00\x00\x00\x00avifmif1miaf";

const AVIF_COMPATIBLE: &[u8] = b"\x00\x00\x00\x1cftypmif1\x00\x00\x00\x00mif1avifmiaf";

fn held(content_type: &str, bytes: &[u8]) -> Option<String> {
    image(Some(content_type), bytes.to_vec()).map(|image| image.media_type)
}

#[test]
fn each_raster_type_passes_with_the_bytes_a_file_of_it_begins_with() {
    assert_eq!(held("image/png", PNG).as_deref(), Some("image/png"));
    assert_eq!(held("image/jpeg", JPEG).as_deref(), Some("image/jpeg"));
    assert_eq!(
        held("image/gif", b"GIF89a....").as_deref(),
        Some("image/gif")
    );
    assert_eq!(
        held("image/gif", b"GIF87a....").as_deref(),
        Some("image/gif")
    );
    assert_eq!(held("image/webp", WEBP).as_deref(), Some("image/webp"));
    assert_eq!(held("image/avif", AVIF).as_deref(), Some("image/avif"));
    assert_eq!(
        held("image/avif", AVIF_COMPATIBLE).as_deref(),
        Some("image/avif")
    );
}

#[test]
fn the_type_is_read_without_its_parameters_and_in_any_case() {
    assert_eq!(
        held("image/png; charset=binary", PNG).as_deref(),
        Some("image/png")
    );
    assert_eq!(held(" IMAGE/PNG ", PNG).as_deref(), Some("image/png"));
    assert_eq!(raster("Image/JPEG;q=1"), Some("image/jpeg"));
}

#[test]
fn an_svg_or_any_type_that_is_not_a_raster_one_is_refused() {
    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg>"#;
    for named in [
        "image/svg+xml",
        "image/svg",
        "text/html",
        "image/",
        "image/x-icon",
        "application/octet-stream",
        "",
    ] {
        assert_eq!(held(named, svg), None, "{named}");
        assert_eq!(held(named, PNG), None, "{named}");
    }
    assert!(image(None, PNG.to_vec()).is_none());
}

#[test]
fn bytes_that_do_not_begin_as_the_type_they_are_served_as_are_refused() {
    let page = b"<html><script>alert(1)</script></html>";
    assert_eq!(held("image/png", page), None);
    assert_eq!(held("image/jpeg", PNG), None);
    assert_eq!(held("image/png", JPEG), None);
    assert_eq!(held("image/webp", b"RIFF\x24\x00\x00\x00WAVEfmt "), None);
    assert_eq!(held("image/gif", b"GIF90a"), None);
    assert_eq!(held("image/png", &[]), None);
}

#[test]
fn an_iso_media_file_is_avif_only_where_its_ftyp_box_names_the_brand() {
    let heic = b"\x00\x00\x00\x18ftypheic\x00\x00\x00\x00mif1heic";
    assert_eq!(held("image/avif", heic), None);
    let elsewhere = b"\x00\x00\x00\x10ftypmif1\x00\x00\x00\x00avif";
    assert_eq!(held("image/avif", elsewhere), None);
    let no_box = b"\x00\x00\x00\x1cmoovavif\x00\x00\x00\x00avif";
    assert_eq!(held("image/avif", no_box), None);
    let short = b"\x00\x00\x00\x1cftyp";
    assert_eq!(held("image/avif", short), None);
    let oversized = b"\xff\xff\xff\xffftypmif1\x00\x00\x00\x00avif";
    assert_eq!(held("image/avif", oversized).as_deref(), Some("image/avif"));
}
