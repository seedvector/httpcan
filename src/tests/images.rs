//! Images: /image /image/{fmt} and Accept negotiation

use super::*;

/// GET /image/png returns 200 with content-type image/png and a body whose
/// first bytes are the PNG magic \x89PNG.
#[actix_web::test]
async fn img_png_returns_valid_png() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/image/png").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "image/png"
    );
    let body = test::read_body(resp).await;
    assert!(!body.is_empty(), "png body must not be empty");
    assert!(
        body.starts_with(&[0x89, b'P', b'N', b'G']),
        "png body must start with PNG magic bytes, got: {:?}",
        &body[..body.len().min(8)]
    );
}

/// GET /image/jpeg returns 200 with content-type image/jpeg and a body whose
/// first bytes are the JPEG magic 0xFF 0xD8 0xFF.
#[actix_web::test]
async fn img_jpeg_returns_valid_jpeg() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/image/jpeg").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "image/jpeg"
    );
    let body = test::read_body(resp).await;
    assert!(!body.is_empty(), "jpeg body must not be empty");
    assert!(
        body.starts_with(&[0xff, 0xd8, 0xff]),
        "jpeg body must start with JPEG magic bytes, got: {:?}",
        &body[..body.len().min(4)]
    );
}

/// GET /image/webp returns 200 with content-type image/webp and a body that
/// carries the RIFF/WEBP container markers.
#[actix_web::test]
async fn img_webp_returns_valid_webp() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/image/webp").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "image/webp"
    );
    let body = test::read_body(resp).await;
    assert!(body.len() > 12, "webp body must be at least 13 bytes");
    assert_eq!(&body[..4], b"RIFF", "webp body must start with RIFF marker");
    assert_eq!(
        &body[8..12],
        b"WEBP",
        "webp body must carry WEBP fourcc at offset 8..12"
    );
}

/// GET /image/svg returns 200 with content-type image/svg+xml and a body that
/// is UTF-8 text containing the <svg root element.
#[actix_web::test]
async fn img_svg_returns_valid_svg() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/image/svg").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "image/svg+xml"
    );
    let body = test::read_body(resp).await;
    assert!(!body.is_empty(), "svg body must not be empty");
    let text = std::str::from_utf8(&body).expect("svg body must be valid UTF-8");
    assert!(
        text.contains("<svg"),
        "svg body must contain the <svg element"
    );
}

/// GET /image with Accept: image/png is negotiated to image/png.
#[actix_web::test]
async fn image_accept_png_negotiated() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/image")
        .insert_header(("Accept", "image/png"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "image/png"
    );
    let body = test::read_body(resp).await;
    assert!(
        body.starts_with(&[0x89, b'P', b'N', b'G']),
        "negotiated png body must start with PNG magic bytes"
    );
}

/// GET /image with Accept: */* returns a 200 whose content-type is some image
/// type (the handler picks a random format).
#[actix_web::test]
async fn image_accept_wildcard_random_image_type() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get()
        .uri("/image")
        .insert_header(("Accept", "*/*"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp
        .headers()
        .get("content-type")
        .expect("content-type header present")
        .to_str()
        .unwrap()
        .to_owned();
    assert!(
        ct.starts_with("image/"),
        "default /image content-type must be an image type, got: {ct}"
    );
}

/// GET /image with no Accept header at all behaves like the default random path.
#[actix_web::test]
async fn image_no_accept_header_random_image_type() {
    let app = test::init_service(create_app(cfg())).await;
    let req = test::TestRequest::get().uri("/image").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp
        .headers()
        .get("content-type")
        .expect("content-type header present")
        .to_str()
        .unwrap()
        .to_owned();
    assert!(
        ct.starts_with("image/"),
        "default /image content-type must be an image type, got: {ct}"
    );
}
