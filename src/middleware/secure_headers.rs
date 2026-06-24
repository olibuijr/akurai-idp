use axum::{
    body::Body,
    http::{Request, Response},
    middleware::Next,
};

pub async fn secure_headers(request: Request<Body>, next: Next) -> Response<Body> {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();

    headers.insert("x-frame-options", "DENY".parse().unwrap());
    headers.insert("x-content-type-options", "nosniff".parse().unwrap());
    headers.insert(
        "strict-transport-security",
        "max-age=31536000; includeSubDomains".parse().unwrap(),
    );
    headers.insert("referrer-policy", "strict-origin-when-cross-origin".parse().unwrap());
    headers.insert(
        "permissions-policy",
        "camera=(), microphone=(), geolocation=(), interest-cohort=()"
            .parse()
            .unwrap(),
    );
    headers.insert(
        "content-security-policy",
        "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; frame-ancestors 'none'; form-action 'self'"
            .parse()
            .unwrap(),
    );
    headers.insert("x-xss-protection", "0".parse().unwrap());

    response
}
