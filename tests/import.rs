use rustload::{core::*, import::*};

fn one(text: &str) -> (Request, Vec<String>) {
    let mut imported = from_curl(text, 7).unwrap();
    assert_eq!(imported.requests.len(), 1, "{:?}", imported.warnings);
    (imported.requests.remove(0), imported.warnings)
}
fn header<'a>(r: &'a Request, name: &str) -> Option<&'a Field> {
    r.headers.iter().find(|h| h.name.eq_ignore_ascii_case(name))
}
fn query<'a>(r: &'a Request, name: &str) -> Option<&'a Field> {
    r.query.iter().find(|q| q.name == name)
}

#[test]
fn chrome_copy_as_curl_bash() {
    let text = r#"curl 'https://api.example.com/v1/login?next=%2Fhome&token=abc#frag' \
  -H 'accept: application/json' \
  -H 'accept-encoding: gzip, deflate, br, zstd' \
  -H 'authorization: Bearer secret-token' \
  -H 'content-type: application/json' \
  -H 'cookie: session=xyz' \
  --data-raw '{"user":"a","pass":"b"}' \
  --compressed"#;
    let (r, warnings) = one(text);
    assert_eq!(r.id, 7);
    assert_eq!(r.name, "/v1/login");
    assert_eq!(r.method, "POST");
    assert_eq!(r.url, "https://api.example.com/v1/login");
    assert_eq!(query(&r, "next").unwrap().value, "/home");
    let token = query(&r, "token").unwrap();
    assert!(token.secret && token.value == "abc");
    assert!(header(&r, "accept-encoding").is_none());
    assert!(
        header(&r, "content-type").is_none(),
        "JSON bodies set it themselves"
    );
    assert!(r.body.json && !r.body.persist);
    assert_eq!(r.body.text, r#"{"user":"a","pass":"b"}"#);
    assert!(header(&r, "authorization").unwrap().secret);
    assert!(header(&r, "cookie").unwrap().secret);
    assert!(!header(&r, "accept").unwrap().secret);
    assert!(warnings.iter().any(|w| w.contains("Accept-Encoding")));
}

#[test]
fn quoting_forms_browsers_emit() {
    // bash ANSI-C strings, as Chrome writes for values with quotes or non-ASCII.
    let (r, _) = one(r#"curl $'https://x.test/a' --data-raw $'{"msg":"it\'s é\\n"}'"#);
    assert_eq!(r.body.text, "{\"msg\":\"it's é\\n\"}");
    // Double quotes with escapes, repeated -d joined with &.
    let (r, _) = one(r#"curl "https://x.test/b" -H "X-Note: say \"hi\"" -d "a=1" -d 'b=2'"#);
    assert_eq!(
        (r.method.as_str(), r.body.text.as_str()),
        ("POST", "a=1&b=2")
    );
    assert_eq!(header(&r, "x-note").unwrap().value, r#"say "hi""#);
    // Firefox style explicit method with a non-JSON body keeps its content type.
    let (r, _) = one(
        "curl 'https://x.test/items' -X 'PUT' -H 'Content-Type: text/plain' --data-raw 'hello'",
    );
    assert_eq!(r.method, "PUT");
    assert!(!r.body.json);
    assert_eq!(header(&r, "content-type").unwrap().value, "text/plain");
    // Attached short values and --name=value.
    let (r, _) = one("curl -XPATCH --url=https://x.test/c -H'X-A: 1'");
    assert_eq!(
        (r.method.as_str(), r.url.as_str()),
        ("PATCH", "https://x.test/c")
    );
    assert_eq!(header(&r, "x-a").unwrap().value, "1");
    assert!(from_curl("curl 'https://x.test/open", 1).is_err());
}

#[test]
fn query_auth_json_and_get_options() {
    let (r, _) = one("curl -G https://x.test/search -d q=rust -d page=2");
    assert_eq!(r.method, "GET");
    assert!(r.body.text.is_empty());
    assert_eq!(query(&r, "q").unwrap().value, "rust");
    assert_eq!(query(&r, "page").unwrap().value, "2");

    let (r, _) = one("curl -u alice:pw https://x.test/p");
    assert_eq!(r.auth.kind, AuthKind::Basic);
    assert_eq!(
        (r.auth.username.as_str(), r.auth.password.as_str()),
        ("alice", "pw")
    );
    let (r, _) = one("curl https://bob:se%40cret@x.test/q");
    assert_eq!(r.url, "https://x.test/q");
    assert_eq!(
        (r.auth.username.as_str(), r.auth.password.as_str()),
        ("bob", "se@cret")
    );

    let (r, _) = one(r#"curl --json '{"a":1}' https://x.test/j"#);
    assert_eq!(r.method, "POST");
    assert!(r.body.json);
    assert!(header(&r, "content-type").is_none());
    assert_eq!(header(&r, "accept").unwrap().value, "application/json");

    let (r, _) = one("curl --data-urlencode 'msg=a b&c' https://x.test/u");
    assert_eq!(r.body.text, "msg=a+b%26c");
}

#[test]
fn copy_all_unsupported_options_and_limits() {
    let text = "curl 'https://x.test/1' ;\ncurl 'https://x.test/2' -I ;\ncurl 'https://x.test/3' -k -L -F 'f=@a.txt' --foo ;\necho done";
    let imported = from_curl(text, 1).unwrap();
    let ids: Vec<_> = imported
        .requests
        .iter()
        .map(|r| (r.id, r.url.as_str()))
        .collect();
    assert_eq!(ids, [(1, "https://x.test/1"), (2, "https://x.test/3")]);
    let w = imported.warnings.join("\n");
    for expected in [
        "HEAD is not supported",
        "TLS certificates are always verified",
        "redirects are never followed",
        "multipart and file uploads are not supported",
        "Ignored option --foo",
        "not a curl command: echo",
    ] {
        assert!(w.contains(expected), "missing {expected:?} in {w}");
    }
    assert!(from_curl("wget https://x.test", 1).is_err());
    assert!(
        from_curl("curl ftp://x.test/file", 1)
            .unwrap()
            .requests
            .is_empty()
    );

    let many: String = (0..=IMPORT_LIMIT)
        .map(|i| format!("curl https://x.test/{i}\n"))
        .collect();
    let imported = from_curl(&many, 1).unwrap();
    assert_eq!(imported.requests.len(), IMPORT_LIMIT);
    assert!(imported.warnings.iter().any(|w| w.contains("plan limit")));
}

#[test]
fn imported_plans_validate_and_never_save_credentials() {
    let text = "curl 'https://x.test/api?api_key=k1' -H 'Authorization: Bearer t0k' -H 'Cookie: s=1' -u u:p";
    let imported = from_curl(text, 1).unwrap();
    let plan = TestPlan {
        requests: imported.requests,
        ..TestPlan::default()
    };
    assert!(plan.validate(std::path::Path::new(".")).is_ok());
    let saved = serde_json::to_string(&plan).unwrap();
    for secret in ["k1", "t0k", "s=1", "\"p\""] {
        assert!(!saved.contains(secret), "{secret} was saved");
    }
}

#[test]
fn har_requests_in_order_with_filters() {
    let har = r#"{"log":{"version":"1.2","entries":[
      {"_resourceType":"document","request":{"method":"GET","url":"https://x.test/","headers":[]},
       "response":{"content":{"text":"<html>private</html>"}}},
      {"_resourceType":"fetch","request":{"method":"POST","url":"https://x.test/api/login?x=1",
       "headers":[{"name":":authority","value":"x.test"},{"name":"Content-Length","value":"9"},
                  {"name":"Content-Type","value":"application/json"},{"name":"Cookie","value":"sid=9"}],
       "postData":{"mimeType":"application/json","text":"{\"a\":true}"}}},
      {"request":{"method":"POST","url":"https://x.test/form","headers":[],
       "postData":{"mimeType":"application/x-www-form-urlencoded","params":[{"name":"a b","value":"1&2"}]}}},
      {"_resourceType":"websocket","request":{"method":"GET","url":"wss://x.test/live","headers":[]}},
      {"_resourceType":"xhr","request":{"method":"OPTIONS","url":"https://x.test/api","headers":[]}}
    ]}}"#;
    let imported = from_har(har.as_bytes(), 3, true).unwrap();
    let got: Vec<_> = imported
        .requests
        .iter()
        .map(|r| (r.id, r.method.as_str(), r.url.as_str()))
        .collect();
    assert_eq!(
        got,
        [
            (3, "POST", "https://x.test/api/login"),
            (4, "POST", "https://x.test/form")
        ]
    );
    let login = &imported.requests[0];
    assert!(login.body.json);
    assert_eq!(login.headers.len(), 1);
    assert!(header(login, "cookie").unwrap().secret);
    assert_eq!(query(login, "x").unwrap().value, "1");
    assert_eq!(imported.requests[1].body.text, "a+b=1%262");
    let w = imported.warnings.join("\n");
    assert!(w.contains("Skipped 2 non-API entries"), "{w}");
    assert!(w.contains("OPTIONS is not supported"), "{w}");
    // Without the filter the document is kept too; websocket still is not HTTP.
    let all = from_har(har.as_bytes(), 1, false).unwrap();
    assert_eq!(all.requests.len(), 3);
    assert!(all.warnings.iter().any(|w| w.contains("not HTTP(S)")));
    assert!(from_har(b"{}", 1, true).is_err());
}
