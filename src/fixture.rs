//! Local-only acceptance server. Never binds to a public interface.
use std::{
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
use tokio_util::sync::CancellationToken;

pub struct Fixture {
    pub address: SocketAddr,
    pub stop: CancellationToken,
    pub count: Arc<AtomicU64>,
}
pub async fn spawn(port: u16) -> std::io::Result<Fixture> {
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).await?;
    let address = listener.local_addr()?;
    let stop = CancellationToken::new();
    let cancel = stop.clone();
    let count = Arc::new(AtomicU64::new(0));
    let requests = count.clone();
    tokio::spawn(async move {
        loop {
            let accepted =
                tokio::select! { _=cancel.cancelled()=>break, result=listener.accept()=>result };
            let Ok((mut socket, _)) = accepted else {
                break;
            };
            let count = requests.clone();
            let cancel = cancel.clone();
            tokio::spawn(async move {
                let exchange = async {
                    let mut buffer = Vec::new();
                    let mut chunk = [0_u8; 4096];
                    let header_end = loop {
                        let n = socket.read(&mut chunk).await?;
                        if n == 0 {
                            return Ok::<(), std::io::Error>(());
                        }
                        buffer.extend_from_slice(&chunk[..n]);
                        if let Some(end) = buffer.windows(4).position(|w| w == b"\r\n\r\n") {
                            break end + 4;
                        }
                        if buffer.len() > 65536 {
                            return Ok(());
                        }
                    };
                    let header = String::from_utf8_lossy(&buffer[..header_end]).to_string();
                    let length = header
                        .lines()
                        .find_map(|l| {
                            l.split_once(':')
                                .filter(|(k, _)| k.eq_ignore_ascii_case("content-length"))
                                .and_then(|(_, v)| v.trim().parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if length > 1024 * 1024 {
                        return Ok(());
                    }
                    while buffer.len() < header_end + length {
                        let n = socket.read(&mut chunk).await?;
                        if n == 0 {
                            return Ok(());
                        }
                        buffer.extend_from_slice(&chunk[..n]);
                    }
                    count.fetch_add(1, Ordering::Relaxed);
                    let path = header
                        .lines()
                        .next()
                        .and_then(|s| s.split_whitespace().nth(1))
                        .unwrap_or("/")
                        .split('?')
                        .next()
                        .unwrap_or("/");
                    let authorized = header
                        .to_ascii_lowercase()
                        .contains("authorization: bearer local-fixture-token");
                    let (status, body) = match path {
                        "/identity-login" => {
                            let user=serde_json::from_slice::<serde_json::Value>(&buffer[header_end..header_end+length]).ok().and_then(|v|v.get("user").and_then(|u|u.as_str()).map(str::to_owned)).unwrap_or_default();
                            (200,serde_json::json!({"access_token":format!("user-{user}")}).to_string())
                        }
                        "/identity-check" => {
                            let target=header.lines().next().and_then(|s|s.split_whitespace().nth(1)).unwrap_or_default();
                            let user=url::Url::parse(&format!("http://localhost{target}")).ok().and_then(|u|u.query_pairs().find(|(k,_)|k=="user").map(|(_,v)|v.into_owned())).unwrap_or_default();
                            let expected=format!("authorization: bearer user-{user}");
                            let correct=header.lines().any(|l|l.to_ascii_lowercase()==expected);
                            (if correct {200}else{403},serde_json::json!({"ok":correct}).to_string())
                        }
                        "/echo" => {
                            let first=header.lines().next().unwrap_or_default();
                            let mut parts=first.split_whitespace();
                            let method=parts.next().unwrap_or_default();let target=parts.next().unwrap_or_default();
                            let get=|key:&str|header.lines().find_map(|l|l.split_once(':').filter(|(name,_)|name.eq_ignore_ascii_case(key)).map(|(_,v)|v.trim())).unwrap_or_default();
                            (200,serde_json::json!({"request":{"method":method,"target":target,"content_type":get("content-type"),"authorization":get("authorization"),"custom":get("x-custom"),"body":String::from_utf8_lossy(&buffer[header_end..header_end+length])}}).to_string())
                        }
                        "/redirect" => {socket.write_all(b"HTTP/1.1 302 Found\r\nLocation: /health\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await?;return Ok(());}
                        "/cookie" => {socket.write_all(b"HTTP/1.1 200 OK\r\nSet-Cookie: session=private\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}").await?;return Ok(());}
                        "/no-cookie" => (200,serde_json::json!({"cookie_absent":!header.lines().any(|l|l.to_ascii_lowercase().starts_with("cookie:"))}).to_string()),
                        "/login" => (200, r#"{"access_token":"local-fixture-token"}"#.to_owned()),
                        "/products" if !authorized => (401, r#"{"error":"unauthorized"}"#.into()),
                        "/products" => (200, r#"{"products":[{"id":"product-1"}]}"#.into()),
                        "/checkout" => {
                            if serde_json::from_slice::<serde_json::Value>(
                                &buffer[header_end..header_end + length],
                            )
                            .ok()
                            .is_some_and(|v| v.get("product_id").is_some())
                            {
                                (201, r#"{"created":true}"#.into())
                            } else {
                                (400, r#"{"error":"product_id required"}"#.into())
                            }
                        }
                        "/slow" => {
                            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                            (200, "{}".into())
                        }
                        "/body-delay" => {
                            socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n").await?;
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                            socket.write_all(b"{}").await?;
                            return Ok(());
                        }
                        "/large" => (200, "x".repeat(16384)),
                        "/malformed" => (200, "not-json".into()),
                        "/disconnect" => return Ok(()),
                        "/status/429" => (429, "{}".into()),
                        "/status/500" => (500, "{}".into()),
                        _ => (200, r#"{"ok":true}"#.into()),
                    };
                    let response = format!(
                        "HTTP/1.1 {status} Response\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    socket.write_all(response.as_bytes()).await?;
                    Ok(())
                };
                tokio::select! { _=cancel.cancelled()=>{}, _=tokio::time::timeout(std::time::Duration::from_secs(15),exchange)=>{} }
            });
        }
    });
    Ok(Fixture {
        address,
        stop,
        count,
    })
}

pub fn demo_plan(address: SocketAddr) -> crate::core::TestPlan {
    use crate::core::*;
    let base = format!("http://{address}");
    let login = Request {
        id: 1,
        name: "Login".into(),
        method: "POST".into(),
        url: format!("{base}/login"),
        extractors: vec![Extractor {
            variable: "access_token".into(),
            pointer: "/access_token".into(),
        }],
        ..Request::default()
    };
    let products = Request {
        id: 2,
        name: "List products".into(),
        url: format!("{base}/products"),
        auth: Auth {
            kind: AuthKind::Bearer,
            bearer: "{{access_token}}".into(),
            ..Auth::default()
        },
        ..Request::default()
    };
    let checkout = Request {
        id: 3,
        name: "Checkout".into(),
        method: "POST".into(),
        url: format!("{base}/checkout"),
        body: Body {
            text: r#"{"product_id":"{{product_id}}"}"#.into(),
            json: true,
            persist: true,
        },
        assertions: Assertions {
            status: Some(201),
            ..Assertions::default()
        },
        ..Request::default()
    };
    TestPlan {
        name: "Local API · Checkout scenario".into(),
        variables: vec![Field {
            name: "product_id".into(),
            value: "product-1".into(),
            secret: false,
        }],
        requests: vec![login, products, checkout],
        ..TestPlan::default()
    }
}

/// Public demo API that Grafana k6 provides for load-test practice.
pub const K6_DEMO_ORIGIN: &str = "https://quickpizza.grafana.com";

/// Ready-made plan for Grafana k6's public QuickPizza demo: four requests with
/// status, response-time and JSON assertions, an extraction, warmup and SLOs.
/// QuickPizza accepts any 16-character API token; `token` is kept in a secret
/// variable, so it is never written to saved plans.
pub fn k6_demo_plan(token: &str) -> crate::core::TestPlan {
    use crate::core::*;
    let auth = vec![Field {
        name: "Authorization".into(),
        value: "{{pizza_token}}".into(),
        secret: true,
    }];
    let checked = |status: u16, max_ms: u64| Assertions {
        status: Some(status),
        max_ms: Some(max_ms),
        ..Assertions::default()
    };
    let home = Request {
        id: 1,
        name: "Home".into(),
        url: format!("{K6_DEMO_ORIGIN}/"),
        assertions: checked(200, 1000),
        ..Request::default()
    };
    let quotes = Request {
        id: 2,
        name: "Quotes".into(),
        url: format!("{K6_DEMO_ORIGIN}/api/quotes"),
        assertions: checked(200, 1000),
        ..Request::default()
    };
    let doughs = Request {
        id: 3,
        name: "Doughs".into(),
        url: format!("{K6_DEMO_ORIGIN}/api/doughs"),
        headers: auth.clone(),
        assertions: Assertions {
            json_pointer: "/doughs/0/name".into(),
            json_expected: r#""Thin""#.into(),
            ..checked(200, 1000)
        },
        ..Request::default()
    };
    let recommend = Request {
        id: 4,
        name: "Recommend pizza".into(),
        method: "POST".into(),
        url: format!("{K6_DEMO_ORIGIN}/api/pizza"),
        headers: auth,
        body: Body {
            text: r#"{"maxCaloriesPerSlice":1000,"mustBeVegetarian":false,"excludedIngredients":[],"excludedTools":[],"maxNumberOfToppings":5,"minNumberOfToppings":2}"#.into(),
            json: true,
            persist: true,
        },
        assertions: checked(200, 1500),
        extractors: vec![Extractor {
            variable: "pizza_name".into(),
            pointer: "/pizza/name".into(),
        }],
        ..Request::default()
    };
    let threshold = |metric, value| Threshold {
        metric,
        value,
        request_id: None,
    };
    TestPlan {
        name: "k6 Demo · QuickPizza".into(),
        variables: vec![Field {
            name: "pizza_token".into(),
            value: format!("token {token}"),
            secret: true,
        }],
        requests: vec![home, quotes, doughs, recommend],
        load: LoadConfig {
            users: 10,
            duration_secs: 70,
            ramp_secs: 10,
            profile: Profile::RampUp,
            rps_limit: 20,
            think_ms: 1000,
            warmup: Warmup::Duration { secs: 10 },
            ..LoadConfig::default()
        },
        thresholds: vec![
            threshold(SloMetric::P95, 800.0),
            threshold(SloMetric::P99, 1500.0),
            threshold(SloMetric::ErrorRate, 1.0),
            threshold(SloMetric::Throughput, 10.0),
        ],
        ..TestPlan::default()
    }
}
