use tokio::sync::Mutex;
use std::sync::Arc;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Server, Body, Request, Method, Response, StatusCode};

fn generate_top_page(n: usize) -> String {
    let head = r#"<html>
    <head><title>toy-web-counter</title></head>
    <body>"#;
    let tail = r#"<form action="/count-up" method="post">
      <div><button name="diff" value="1">Count Up</button></div>
    </form></body></html>"#;
    let message = format!("<p>Count = {}</p>", n);
    format!("{}\n{}\n{}", head, message, tail)
}

fn generate_countup_page(n: usize) -> String {
    let head = r#"<html>
    <head><title>toy-web-counter</title></head>
    <body>"#;
    let tail = r#"<a href="/">Back to Top Page</a>
        </body></html>"#;
    let message = format!("<p>Count = {}</p>", n);
    format!("{}\n{}\n{}", head, message, tail)
}

fn parse_parameter(param: &str) -> Option<usize> {
    let first = param.split('&').nth(0)?;
    let mut it = first.split('=');
    let key = it.next()?;
    let val = it.next()?;
    match key {
        "diff" => {
            match val.parse::<usize>() {
                Ok(n) => Some(n),
                Err(_) => None,
            }
        },
        _ => None,
    }
}

fn response_bad_request() -> Response<Body> {
    Response::builder()
        .status(StatusCode::BAD_REQUEST)
        .body("".into())
        .unwrap()
}

fn response_not_found() -> Response<Body> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body("".into())
        .unwrap()
}

async fn counter(count: Arc<Mutex<usize>>, req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    let (parts, body) = req.into_parts();

    match (parts.method, parts.uri.path()) {
        (Method::GET, "/") => {
            let n = {
                let n = count.lock().await;
                n.clone()
            };
            let page = generate_top_page(n);
            let response = Response::builder()
                .header("Content-Type", "text/html")
                .header("Content-Length", page.len())
                .body(page.into())
                .unwrap();
            Ok(response)
        },
        (Method::GET, _) => Ok(response_not_found()),
        (Method::POST, "/count-up") => {
            let bytes = hyper::body::to_bytes(body).await?;
            let param = match std::str::from_utf8(&bytes) {
                Ok(param) => param,
                Err(_) => return Ok(response_bad_request()),
            };
            if let Some(diff) = parse_parameter(param) {
                let n = {
                    let mut n = count.lock().await;
                    *n += diff;
                    n.clone()
                };
                let page = generate_countup_page(n);
                let response = Response::builder()
                    .header("Content-Type", "text/html")
                    .header("Content-Length", page.len())
                    .body(page.into())
                    .unwrap();
                Ok(response)
            } else {
                Ok(response_bad_request())
            }
        }
        _ => Ok(response_not_found()),
    }
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install CTRL+C signal handler");
}

#[tokio::main]
async fn main() {
    let count = Arc::new(Mutex::new(0usize));

    let addr = ([127, 0, 0, 1], 3000).into();

    let service = make_service_fn(move |_| {
        let count = count.clone();

        async move {
            Ok::<_, hyper::Error>(service_fn(move |req| {
                let count = count.clone();
                counter(count, req)
            }))
        }
    });

    let server = Server::bind(&addr).serve(service);
    let graceful = server.with_graceful_shutdown(shutdown_signal());

    println!("Listening on http://{}", addr);

    if let Err(e) = graceful.await {
        eprintln!("server error: {}", e);
    }
}
