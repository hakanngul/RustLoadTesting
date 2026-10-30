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

const PETSTORE: &str = r##"{
  "openapi": "3.0.3",
  "servers": [{"url": "https://{env}.example.com/v1", "variables": {"env": {"default": "api"}}}],
  "security": [{"bearerAuth": []}],
  "paths": {
    "/pets/{petId}": {
      "parameters": [{"$ref": "#/components/parameters/PetId"}],
      "get": {
        "operationId": "getPet",
        "parameters": [
          {"name": "fields", "in": "query", "required": true, "schema": {"type": "string", "default": "name"}},
          {"name": "verbose", "in": "query", "schema": {"type": "boolean"}},
          {"name": "X-Request-Id", "in": "header", "required": true, "example": "abc"},
          {"name": "session", "in": "cookie", "required": true}
        ]
      },
      "delete": {"summary": "Remove a pet", "security": []},
      "head": {"operationId": "petExists"}
    },
    "/pets": {
      "post": {
        "operationId": "createPet",
        "requestBody": {"$ref": "#/components/requestBodies/Pet"}
      },
      "put": {
        "operationId": "uploadPhoto",
        "requestBody": {"content": {"image/png": {}}}
      }
    },
    "/owners/{owner-name}": {
      "get": {"parameters": [{"name": "owner-name", "in": "path", "required": true}]}
    }
  },
  "components": {
    "parameters": {"PetId": {"name": "petId", "in": "path", "required": true, "schema": {"type": "integer", "example": 42}}},
    "requestBodies": {"Pet": {"content": {"application/json": {"schema": {"$ref": "#/components/schemas/Pet"}}}}},
    "schemas": {"Pet": {"type": "object", "properties": {
      "name": {"type": "string", "example": "Rex"},
      "tags": {"type": "array", "items": {"type": "string", "enum": ["dog", "cat"]}},
      "age": {"type": "integer"}
    }}},
    "securitySchemes": {"bearerAuth": {"type": "http", "scheme": "bearer"}}
  }
}"##;

#[test]
fn openapi3_operations_become_requests() {
    let imported = from_openapi(PETSTORE.as_bytes(), 5, "").unwrap();
    let by_name = |name: &str| {
        imported
            .requests
            .iter()
            .find(|r| r.name == name)
            .unwrap_or_else(|| panic!("{name} missing: {:?}", imported.requests))
    };
    // HEAD is skipped; IDs start at the requested first ID.
    assert_eq!(imported.requests.len(), 5);
    assert_eq!(imported.requests[0].id, 5);

    let get = by_name("getPet");
    assert_eq!(get.method, "GET");
    assert_eq!(get.url, "https://api.example.com/v1/pets/{{petId}}");
    assert_eq!(query(get, "fields").unwrap().value, "name");
    assert!(query(get, "verbose").is_none(), "optional without example");
    assert_eq!(header(get, "X-Request-Id").unwrap().value, "abc");
    assert_eq!(get.auth.kind, AuthKind::Bearer);
    assert_eq!(get.auth.bearer, "{{access_token}}");

    // An empty operation security list overrides the global requirement.
    let delete = by_name("Remove a pet");
    assert_eq!(delete.auth.kind, AuthKind::None);

    let post = by_name("createPet");
    assert!(post.body.json);
    let body: serde_json::Value = serde_json::from_str(&post.body.text).unwrap();
    // Document examples where present; no property is required, so all are
    // included and `age` gets a type placeholder.
    assert_eq!(
        body,
        serde_json::json!({"name": "Rex", "tags": ["dog"], "age": 0})
    );
    assert!(by_name("uploadPhoto").body.text.is_empty());

    let owner = by_name("/owners/{owner-name}");
    assert_eq!(
        owner.url,
        "https://api.example.com/v1/owners/{{owner_name}}"
    );

    let var = |name: &str| imported.variables.iter().find(|v| v.name == name).unwrap();
    assert_eq!(var("petId").value, "42");
    assert!(!var("petId").secret);
    assert_eq!(var("owner_name").value, "");
    assert!(var("access_token").secret && var("access_token").value.is_empty());
    let warnings = imported.warnings.join("\n");
    for expected in [
        "path variables without an example: owner_name",
        "{{access_token}}",
        "1 HEAD/OPTIONS/TRACE",
        "1 cookie parameters",
        "1 non-JSON request bodies",
        "1 bodies built from schema types",
    ] {
        assert!(warnings.contains(expected), "{expected:?} in {warnings}");
    }
    // The plan accepts them once the secret variable has a value.
    let mut plan = TestPlan {
        requests: imported.requests.clone(),
        variables: imported.variables.clone(),
        ..TestPlan::default()
    };
    assert!(
        plan.validate(std::path::Path::new(".")).is_err(),
        "token required"
    );
    plan.variables
        .iter_mut()
        .find(|v| v.name == "access_token")
        .unwrap()
        .value = "t".into();
    plan.validate(std::path::Path::new(".")).unwrap();
}

#[test]
fn swagger2_and_base_url_rules() {
    let swagger = r##"{
      "swagger": "2.0", "host": "legacy.example.com", "basePath": "/api", "schemes": ["http"],
      "securityDefinitions": {"basic": {"type": "basic"}},
      "paths": {"/orders": {"post": {
        "operationId": "createOrder", "security": [{"basic": []}],
        "parameters": [{"name": "order", "in": "body", "schema": {"example": {"qty": 2}}}]
      }}}
    }"##;
    let imported = from_openapi(swagger.as_bytes(), 1, "").unwrap();
    let r = &imported.requests[0];
    assert_eq!(r.url, "http://legacy.example.com/api/orders");
    assert_eq!(r.auth.kind, AuthKind::Basic);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&r.body.text).unwrap(),
        serde_json::json!({"qty": 2})
    );
    // The base URL field replaces the document's server.
    let local = from_openapi(swagger.as_bytes(), 1, "http://127.0.0.1:8080/").unwrap();
    assert_eq!(local.requests[0].url, "http://127.0.0.1:8080/orders");

    let relative = r#"{"openapi": "3.1.0", "servers": [{"url": "/v2"}], "paths": {}}"#;
    assert!(
        from_openapi(relative.as_bytes(), 1, "")
            .unwrap_err()
            .contains("enter a base URL")
    );
    assert!(from_openapi(relative.as_bytes(), 1, "http://localhost:9").is_ok());
    for bad in ["http://user:pw@host", "ftp://host", "http://host/?a=1"] {
        assert!(from_openapi(relative.as_bytes(), 1, bad).is_err(), "{bad}");
    }
    assert!(
        from_openapi(b"openapi: 3.0.0\npaths: {}", 1, "")
            .unwrap_err()
            .contains("YAML")
    );
    assert!(
        from_openapi(br#"{"info": {}}"#, 1, "")
            .unwrap_err()
            .contains("OpenAPI")
    );
}

fn spec(json: &str) -> OpenApiSpec {
    parse_openapi(json.as_bytes(), "http://127.0.0.1:8080").unwrap()
}
fn keys(list: &[&str]) -> std::collections::BTreeSet<String> {
    list.iter().map(|k| k.to_string()).collect()
}
fn body_of(spec: &OpenApiSpec, key: &str) -> (serde_json::Value, Vec<String>) {
    let imported = spec.build(&keys(&[key]), 1);
    let r = &imported.requests[0];
    assert!(r.body.json, "{key}: {:?}", imported.warnings);
    (
        serde_json::from_str(&r.body.text).unwrap(),
        imported.warnings,
    )
}

const SHOP: &str = r##"{
  "openapi": "3.0.3",
  "servers": [{"url": "https://shop.example.com"}],
  "paths": {
    "/orders": {
      "get": {"operationId": "listOrders", "tags": ["orders"]},
      "post": {"operationId": "createOrder", "tags": ["orders", "write"], "security": [{"bearer": []}],
        "requestBody": {"content": {"application/json": {"schema": {"type": "object", "required": ["qty"],
          "properties": {"qty": {"type": "integer"}, "note": {"type": "string"}}}}}}}
    },
    "/orders/{id}": {
      "parameters": [{"name": "id", "in": "path", "required": true}],
      "get": {"operationId": "getOrder", "tags": ["orders"]},
      "delete": {"summary": "Cancel order", "tags": ["orders", "write"]}
    },
    "/health": {"get": {}, "head": {}}
  },
  "components": {"securitySchemes": {"bearer": {"type": "http", "scheme": "bearer"}}}
}"##;

#[test]
fn openapi_picker_lists_operations_and_builds_the_selection() {
    let spec = spec(SHOP);
    let listed: Vec<_> = spec
        .operations
        .iter()
        .map(|o| (o.key.as_str(), o.name.as_str()))
        .collect();
    // Document order: paths and methods as written.
    assert_eq!(
        listed,
        [
            ("GET /orders", "listOrders"),
            ("POST /orders", "createOrder"),
            ("GET /orders/{id}", "getOrder"),
            ("DELETE /orders/{id}", "Cancel order"),
            ("GET /health", "/health"),
        ]
    );
    assert_eq!(spec.tags(), ["orders", "write"]);
    let create = &spec.operations[1];
    assert_eq!(create.method, "POST");
    assert_eq!(create.path, "/orders");
    assert_eq!(create.operation_id, "createOrder");
    assert_eq!(create.tags, ["orders", "write"]);
    // The filter matches path, name and operationId, ignoring case.
    let matching = |f: &str| -> Vec<&str> {
        spec.operations
            .iter()
            .filter(|o| o.matches(f))
            .map(|o| o.key.as_str())
            .collect()
    };
    assert_eq!(matching("CANCEL"), ["DELETE /orders/{id}"]);
    assert_eq!(matching("createorder"), ["POST /orders"]);
    assert_eq!(matching("{id}").len(), 2);
    assert_eq!(matching("  ").len(), 5);

    // Only selected operations, in document order whatever the key order,
    // with consecutive IDs; variables and warnings follow the selection.
    let some = spec.build(&keys(&["DELETE /orders/{id}", "GET /orders"]), 10);
    let got: Vec<_> = some
        .requests
        .iter()
        .map(|r| (r.id, r.method.as_str(), r.url.as_str()))
        .collect();
    assert_eq!(
        got,
        [
            (10, "GET", "http://127.0.0.1:8080/orders"),
            (11, "DELETE", "http://127.0.0.1:8080/orders/{{id}}")
        ]
    );
    let names: Vec<_> = some.variables.iter().map(|v| v.name.as_str()).collect();
    assert_eq!(names, ["id"], "no access_token without the POST");
    let w = some.warnings.join("\n");
    assert!(w.contains("without an example: id"), "{w}");
    assert!(
        !w.contains("access_token") && !w.contains("schema types"),
        "{w}"
    );
    assert!(w.contains("1 HEAD/OPTIONS/TRACE"), "{w}");
    // Unknown keys select nothing; an empty selection builds nothing.
    assert!(spec.build(&keys(&["PUT /nope"]), 1).requests.is_empty());
    let post = spec.build(&keys(&["POST /orders"]), 1);
    assert_eq!(post.requests[0].auth.bearer, "{{access_token}}");
    assert_eq!(post.variables[0].name, "access_token");
    assert!(
        post.warnings
            .join("\n")
            .contains("1 bodies built from schema types")
    );
    // from_openapi still imports everything.
    assert_eq!(
        from_openapi(SHOP.as_bytes(), 1, "http://localhost:1")
            .unwrap()
            .requests
            .len(),
        5
    );
}

#[test]
fn openapi_selection_is_capped_at_the_plan_limit() {
    let paths: Vec<String> = (0..=IMPORT_LIMIT)
        .map(|i| format!(r#""/r{i:04}": {{"get": {{}}}}"#))
        .collect();
    let doc = format!(
        r#"{{"openapi": "3.0.0", "servers": [{{"url": "http://h"}}], "paths": {{{}}}}}"#,
        paths.join(",")
    );
    let spec = parse_openapi(doc.as_bytes(), "").unwrap();
    assert_eq!(spec.operations.len(), IMPORT_LIMIT + 1, "all are listed");
    let all = spec.operations.iter().map(|o| o.key.clone()).collect();
    let built = spec.build(&all, 1);
    assert_eq!(built.requests.len(), IMPORT_LIMIT);
    assert!(built.warnings.iter().any(|w| w.contains("plan limit")));
}

#[test]
fn openapi_bodies_merge_all_of_and_take_the_first_one_of() {
    let spec = spec(
        r##"{
      "openapi": "3.1.0",
      "paths": {
        "/a": {"post": {"requestBody": {"content": {"application/json": {"schema": {"allOf": [
          {"$ref": "#/components/schemas/Base"},
          {"type": "object", "required": ["kind"], "properties": {
            "kind": {"type": "string", "enum": ["dog", "cat"]},
            "id": {"type": "integer", "example": 7}}},
          {"example": {"extra": true}}
        ]}}}}}},
        "/b": {"post": {"requestBody": {"content": {"application/json": {"schema": {"oneOf": [
          {"$ref": "#/components/schemas/Card"}, {"type": "string"}]}}}}}},
        "/c": {"post": {"requestBody": {"content": {"application/json": {"schema": {"anyOf": [
          {"type": "integer", "minimum": 5}, {"type": "string"}]}}}}}},
        "/d": {"post": {"requestBody": {"content": {"application/json": {"schema": {"allOf": [
          {"$ref": "#/components/schemas/Base"}], "example": {"name": "whole"}}}}}}}
      },
      "components": {"schemas": {
        "Base": {"type": "object", "required": ["name"], "properties": {
          "name": {"type": "string", "example": "Rex"}, "owner": {"type": "string"}}},
        "Card": {"type": "object", "properties": {"number": {"type": "string", "example": "4111"}}}
      }}
    }"##,
    );
    // Required from every member: name (example), kind (enum); id has an
    // example; owner is optional without one; member examples are merged in.
    let (a, warnings) = body_of(&spec, "POST /a");
    assert_eq!(
        a,
        serde_json::json!({"name": "Rex", "kind": "dog", "id": 7, "extra": true})
    );
    assert!(
        !warnings.join("\n").contains("schema types"),
        "{warnings:?}"
    );
    assert_eq!(
        body_of(&spec, "POST /b").0,
        serde_json::json!({"number": "4111"})
    );
    let (c, warnings) = body_of(&spec, "POST /c");
    assert_eq!(c, serde_json::json!(5));
    assert!(
        warnings
            .join("\n")
            .contains("1 bodies built from schema types")
    );
    // A schema-level example wins over its members.
    assert_eq!(
        body_of(&spec, "POST /d").0,
        serde_json::json!({"name": "whole"})
    );
}

#[test]
fn openapi_type_skeletons_use_formats_and_stop_at_recursion() {
    let spec = spec(
        r##"{
      "openapi": "3.0.3",
      "paths": {
        "/user": {"put": {"requestBody": {"content": {"application/json": {"schema": {
          "$ref": "#/components/schemas/User"}}}}}},
        "/tree": {"post": {"requestBody": {"content": {"application/json": {"schema": {
          "$ref": "#/components/schemas/Node"}}}}}},
        "/form": {"post": {"requestBody": {"content": {"application/x-www-form-urlencoded": {
          "schema": {"type": "object"}}}}}},
        "/empty": {"post": {"requestBody": {"content": {"application/json": {}}}}}
      },
      "components": {"schemas": {
        "User": {"type": "object", "required": ["id", "email", "born", "seen", "age", "score", "admin", "roles", "address", "nick"],
          "properties": {
            "id": {"type": "string", "format": "uuid"},
            "email": {"type": "string", "format": "email"},
            "born": {"type": "string", "format": "date"},
            "seen": {"type": "string", "format": "date-time"},
            "age": {"type": "integer", "minimum": 18},
            "score": {"type": "number"},
            "admin": {"type": "boolean"},
            "roles": {"type": "array", "items": {"type": "string"}},
            "address": {"type": "object", "properties": {"city": {"type": "string"}, "zip": {"type": ["string", "null"]}}},
            "nick": {"type": "string", "nullable": true},
            "optional": {"type": "string"}
          }},
        "Node": {"type": "object", "required": ["name", "children", "parent"], "properties": {
          "name": {"type": "string"},
          "children": {"type": "array", "items": {"$ref": "#/components/schemas/Node"}},
          "parent": {"$ref": "#/components/schemas/Node"}
        }}
      }}
    }"##,
    );
    let (user, warnings) = body_of(&spec, "PUT /user");
    assert_eq!(
        user,
        serde_json::json!({
            "id": "00000000-0000-0000-0000-000000000000",
            "email": "user@example.com",
            "born": "1970-01-01",
            "seen": "1970-01-01T00:00:00Z",
            "age": 18,
            "score": 0,
            "admin": false,
            "roles": [""],
            // No required list here, so every property is filled.
            "address": {"city": "", "zip": ""},
            "nick": ""
        })
    );
    assert!(
        warnings
            .join("\n")
            .contains("1 bodies built from schema types; review the values")
    );
    // A recursive schema stops at the first repetition.
    let (tree, _) = body_of(&spec, "POST /tree");
    assert_eq!(tree, serde_json::json!({"name": "", "children": []}));
    let all = spec.build(&spec.operations.iter().map(|o| o.key.clone()).collect(), 1);
    let w = all.warnings.join("\n");
    assert!(w.contains("2 bodies built from schema types"), "{w}");
    assert!(w.contains("1 non-JSON request bodies"), "{w}");
    assert!(w.contains("1 JSON bodies left empty"), "{w}");
}

#[test]
fn openapi_api_keys_become_secret_variable_references() {
    let doc = r##"{
      "openapi": "3.0.3",
      "servers": [{"url": "https://api.example.com"}],
      "security": [{"headerKey": []}],
      "paths": {
        "/a": {"get": {"parameters": [{"name": "X-API-Key", "in": "header", "required": true, "example": "literal-key"}]}},
        "/b": {"get": {"security": [{"cookieKey": []}, {"queryKey": []}]}},
        "/c": {"get": {"security": [{"cookieKey": []}]}},
        "/d": {"get": {"security": [{"headerKey": [], "queryKey": []}]}},
        "/e": {"get": {"security": [{}, {"mystery": []}]}}
      },
      "components": {"securitySchemes": {
        "headerKey": {"type": "apiKey", "in": "header", "name": "X-API-Key"},
        "queryKey": {"type": "apiKey", "in": "query", "name": "key"},
        "cookieKey": {"type": "apiKey", "in": "cookie", "name": "sid"}
      }}
    }"##;
    let imported = from_openapi(doc.as_bytes(), 1, "").unwrap();
    let by_path = |p: &str| {
        imported
            .requests
            .iter()
            .find(|r| r.url.ends_with(p))
            .unwrap()
    };
    // The scheme replaces the document's literal header example.
    let a = by_path("/a");
    assert_eq!(a.headers.len(), 1);
    let key = header(a, "x-api-key").unwrap();
    assert!(key.secret && key.value == "{{api_key}}");
    // The first fully supported alternative wins: the query key, not the cookie.
    let b = by_path("/b");
    let key = query(b, "key").unwrap();
    assert!(key.secret && key.value == "{{api_key_2}}");
    assert!(b.headers.is_empty());
    assert!(by_path("/c").headers.is_empty() && by_path("/c").query.is_empty());
    // Schemes in one requirement are used together.
    let d = by_path("/d");
    assert_eq!(header(d, "X-API-Key").unwrap().value, "{{api_key}}");
    assert_eq!(query(d, "key").unwrap().value, "{{api_key_2}}");
    // An anonymous alternative needs no credentials.
    let e = by_path("/e");
    assert!(e.headers.is_empty() && e.query.is_empty() && e.auth.kind == AuthKind::None);

    let names: Vec<_> = imported
        .variables
        .iter()
        .map(|v| (v.name.as_str(), v.secret, v.value.as_str()))
        .collect();
    assert_eq!(names, [("api_key", true, ""), ("api_key_2", true, "")]);
    let w = imported.warnings.join("\n");
    assert!(w.contains("{{api_key}}, {{api_key_2}}"), "{w}");
    assert!(
        w.contains("cookies are not supported (no cookie support): cookieKey"),
        "{w}"
    );
    assert!(!w.contains("mystery"), "{w}");

    // Saved plans keep the references and never the key values.
    let mut plan = TestPlan {
        requests: imported.requests.clone(),
        variables: imported.variables.clone(),
        ..TestPlan::default()
    };
    assert!(
        plan.validate(std::path::Path::new(".")).is_err(),
        "key required"
    );
    for v in &mut plan.variables {
        v.value = format!("live-{}", v.name);
    }
    plan.validate(std::path::Path::new(".")).unwrap();
    let saved = serde_json::to_string(&plan).unwrap();
    assert!(saved.contains("{{api_key}}") && saved.contains("{{api_key_2}}"));
    assert!(
        !saved.contains("live-") && !saved.contains("literal-key"),
        "{saved}"
    );
    let restored: TestPlan = serde_json::from_str(&saved).unwrap();
    assert_eq!(
        header(&restored.requests[0], "X-API-Key").unwrap().value,
        "{{api_key}}"
    );
    // History snapshots keep them too.
    let redacted = serde_json::to_string(&plan.redacted()).unwrap();
    assert!(redacted.contains("{{api_key_2}}") && !redacted.contains("live-"));

    // Swagger 2.0 apiKey definitions work the same way.
    let swagger = r##"{"swagger": "2.0", "host": "h.example.com",
      "securityDefinitions": {"k": {"type": "apiKey", "in": "header", "name": "X-Token"}},
      "security": [{"k": []}], "paths": {"/s": {"get": {}}}}"##;
    let s = from_openapi(swagger.as_bytes(), 1, "").unwrap();
    assert_eq!(
        header(&s.requests[0], "X-Token").unwrap().value,
        "{{api_key}}"
    );
}

#[test]
fn openapi_operations_follow_document_order() {
    // Paths, methods and properties are deliberately not alphabetical.
    let doc = r##"{
      "openapi": "3.0.3",
      "servers": [{"url": "http://h"}],
      "paths": {
        "/zeta": {"post": {"operationId": "zp", "requestBody": {"content": {"application/json": {
          "schema": {"type": "object", "properties": {"z": {"example": 1}, "a": {"example": 2}, "m": {"example": 3}}}}}}},
          "get": {"operationId": "zg"}},
        "/alpha": {"delete": {"operationId": "ad"}, "get": {"operationId": "ag"}},
        "/mid": {"options": {}, "patch": {"operationId": "mp"}}
      }
    }"##;
    let spec = parse_openapi(doc.as_bytes(), "").unwrap();
    let keys_in_order: Vec<_> = spec.operations.iter().map(|o| o.key.as_str()).collect();
    assert_eq!(
        keys_in_order,
        [
            "POST /zeta",
            "GET /zeta",
            "DELETE /alpha",
            "GET /alpha",
            "PATCH /mid"
        ]
    );
    // A selection is built in document order too, whatever the key order.
    let built = spec.build(&keys(&["PATCH /mid", "GET /alpha", "POST /zeta"]), 1);
    let names: Vec<_> = built.requests.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, ["zp", "ag", "mp"]);
    // Bodies keep the schema's property order.
    let body = &built.requests[0].body.text;
    let (z, a, m) = (
        body.find("\"z\"").unwrap(),
        body.find("\"a\"").unwrap(),
        body.find("\"m\"").unwrap(),
    );
    assert!(z < a && a < m, "{body}");
    assert!(built.warnings.iter().any(|w| w.contains("1 HEAD/OPTIONS")));
}

/// Writes `files` (relative path, contents) under `dir`.
fn write_files(dir: &std::path::Path, files: &[(&str, &str)]) {
    for (name, text) in files {
        let path = dir.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
}

#[test]
fn openapi_external_refs_resolve_relative_to_each_file() {
    let dir = tempfile::tempdir().unwrap();
    write_files(
        dir.path(),
        &[
            (
                "common.json",
                r##"{"components": {"schemas": {
                  "Pet": {"type": "object", "required": ["name", "tag"], "properties": {
                    "name": {"example": "Rex"}, "tag": {"$ref": "#/components/schemas/Tag"}}},
                  "Tag": {"type": "string", "example": "dog"}},
                  "parameters": {"Limit": {"name": "limit", "in": "query", "required": true, "example": 5}}}}"##,
            ),
            (
                "schemas/order.json",
                r##"{"type": "object", "required": ["item", "pet"], "properties": {
                  "item": {"$ref": "item.json"},
                  "pet": {"$ref": "../common.json#/components/schemas/Pet"}}}"##,
            ),
            (
                "schemas/item.json",
                r##"{"type": "integer", "example": 42}"##,
            ),
            (
                "paths/orders.json",
                r##"{"post": {"operationId": "createOrder", "parameters": [
                  {"$ref": "../common.json#/components/parameters/Limit"}],
                  "requestBody": {"content": {"application/json": {
                    "schema": {"$ref": "../schemas/order.json"}}}}}}"##,
            ),
        ],
    );
    let doc = r##"{
      "openapi": "3.0.3",
      "servers": [{"url": "http://h"}],
      "paths": {
        "/pets": {"post": {"operationId": "createPet", "requestBody": {"content": {"application/json": {
          "schema": {"$ref": "common.json#/components/schemas/Pet"}}}}}},
        "/orders": {"$ref": "./paths/orders.json"},
        "/items": {"put": {"operationId": "putItem", "requestBody": {"content": {"application/json": {
          "schema": {"$ref": "schemas/item.json#"}}}}}}
      }
    }"##;
    let spec = parse_openapi_in(doc.as_bytes(), dir.path(), "").unwrap();
    assert!(spec.warnings.is_empty(), "{:?}", spec.warnings);
    // common.json, order.json, item.json and orders.json, each read once.
    assert_eq!(spec.referenced_files, 4);
    let (pet, _) = body_of(&spec, "POST /pets");
    assert_eq!(pet, serde_json::json!({"name": "Rex", "tag": "dog"}));
    let (order, _) = body_of(&spec, "POST /orders");
    assert_eq!(
        order,
        serde_json::json!({"item": 42, "pet": {"name": "Rex", "tag": "dog"}})
    );
    let built = spec.build(&keys(&["POST /orders"]), 1);
    assert_eq!(built.requests[0].name, "createOrder");
    assert_eq!(query(&built.requests[0], "limit").unwrap().value, "5");
    assert_eq!(body_of(&spec, "PUT /items").0, serde_json::json!(42));

    // Without the folder the same document lists the references instead.
    let bytes_only = parse_openapi(doc.as_bytes(), "").unwrap();
    assert_eq!(bytes_only.referenced_files, 0);
    let w = bytes_only.warnings.join("\n");
    assert!(
        w.contains("\"common.json#/components/schemas/Pet\" refers to another file"),
        "{w}"
    );
    // Document warnings are part of every build.
    assert!(
        bytes_only
            .build(&keys(&["POST /pets"]), 1)
            .warnings
            .iter()
            .any(|w| w.contains("./paths/orders.json"))
    );
}

#[test]
fn openapi_refs_across_files_stop_at_cycles() {
    let dir = tempfile::tempdir().unwrap();
    write_files(
        dir.path(),
        &[
            (
                "a.json",
                r##"{"Pet": {"type": "object", "required": ["name", "owner"], "properties": {
                  "name": {"type": "string"}, "owner": {"$ref": "b.json#/Owner"}}}}"##,
            ),
            (
                "b.json",
                r##"{"Owner": {"type": "object", "required": ["pets"], "properties": {
                  "pets": {"type": "array", "items": {"$ref": "a.json#/Pet"}}}},
                  "Loop": {"$ref": "a.json#/Loop"}}"##,
            ),
        ],
    );
    let doc = r##"{"openapi": "3.0.3", "servers": [{"url": "http://h"}], "paths": {
      "/pets": {"post": {"requestBody": {"content": {"application/json": {
        "schema": {"$ref": "a.json#/Pet"}}}}}}}}"##;
    let spec = parse_openapi_in(doc.as_bytes(), dir.path(), "").unwrap();
    assert_eq!(spec.referenced_files, 2);
    let (body, _) = body_of(&spec, "POST /pets");
    // Pet → Owner → [Pet] stops at the repeated Pet.
    assert_eq!(body, serde_json::json!({"name": "", "owner": {"pets": []}}));
    // b.json's own dangling reference is named, not followed forever.
    assert!(
        spec.warnings
            .iter()
            .any(|w| w.contains("\"a.json#/Loop\" points to nothing")),
        "{:?}",
        spec.warnings
    );
}

#[test]
fn openapi_external_refs_stay_local_and_bounded() {
    let outer = tempfile::tempdir().unwrap();
    let dir = outer.path().join("spec");
    let marker = "SECRET-FILE-CONTENT";
    write_files(
        outer.path(),
        &[
            ("outside.json", &format!(r#"{{"x": "{marker}"}}"#)),
            ("spec/ok.json", r#"{"type": "string", "example": "fine"}"#),
            ("spec/notes.yaml", &format!("x: {marker}\n")),
        ],
    );
    // Exactly at the limit is read; one byte more is not.
    let pad = |n: usize| {
        let mut s = String::from("{\"example\": 1, \"pad\": \"");
        s.push_str(&" ".repeat(n - s.len() - 2));
        s.push_str("\"}");
        assert_eq!(s.len(), n);
        s
    };
    let limit = REF_FILE_LIMIT as usize;
    std::fs::write(dir.join("max.json"), pad(limit)).unwrap();
    std::fs::write(dir.join("big.json"), pad(limit + 1)).unwrap();
    for i in 0..REF_FILES_MAX {
        std::fs::write(dir.join(format!("f{i:02}.json")), r#"{"example": 0}"#).unwrap();
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(outer.path().join("outside.json"), dir.join("link.json")).unwrap();

    let schema = |r: &str| {
        format!(r#"{{"content": {{"application/json": {{"schema": {{"$ref": "{r}"}}}}}}}}"#)
    };
    let mut paths = vec![
        ("/ok", "ok.json".to_owned()),
        ("/up", "../outside.json".to_owned()),
        (
            "/abs",
            outer.path().join("outside.json").display().to_string(),
        ),
        ("/link", "link.json".to_owned()),
        ("/url", "https://example.com/pet.json#/Pet".to_owned()),
        ("/scheme", "file:///etc/hosts".to_owned()),
        ("/missing", "missing.json".to_owned()),
        ("/yaml", "notes.yaml".to_owned()),
        ("/max", "max.json".to_owned()),
        ("/big", "big.json".to_owned()),
        ("/nothing", "ok.json#/nope".to_owned()),
        ("/local", "#/components/schemas/Missing".to_owned()),
    ];
    // ok.json and max.json count too: two of these files exceed the limit.
    for i in 0..REF_FILES_MAX {
        paths.push(("/many", format!("f{i:02}.json")));
    }
    let entries: Vec<String> = paths
        .iter()
        .enumerate()
        .map(|(i, (p, r))| {
            format!(
                r#""{p}{i}": {{"post": {{"operationId": "op{i}", "requestBody": {}}}}}"#,
                schema(r)
            )
        })
        .collect();
    let doc = format!(
        r#"{{"openapi": "3.0.3", "servers": [{{"url": "http://h"}}], "paths": {{{}}}}}"#,
        entries.join(",")
    );
    let spec = parse_openapi_in(doc.as_bytes(), &dir, "").unwrap();
    let w = spec.warnings.join("\n");
    let warned = |r: &str, what: &str| {
        assert!(w.contains(&format!("$ref \"{r}\" {what}")), "{r}: {w}");
    };
    warned("../outside.json", "leads outside the document's folder");
    warned(
        &outer.path().join("outside.json").display().to_string(),
        "is an absolute path",
    );
    #[cfg(unix)]
    warned("link.json", "leads outside the document's folder");
    warned(
        "https://example.com/pet.json#/Pet",
        "is a URL and RustLoad never downloads references",
    );
    warned("file:///etc/hosts", "uses an unsupported scheme");
    warned("missing.json", "names a file that does not exist");
    warned(
        "notes.yaml",
        "names a file that is not JSON (YAML is not supported yet)",
    );
    warned("big.json", "names a file larger than 10 MiB");
    warned("ok.json#/nope", "points to nothing");
    warned("#/components/schemas/Missing", "points to nothing");
    warned("f48.json", "would read more than 50 referenced files");
    warned("f49.json", "would read more than 50 referenced files");
    assert!(!w.contains("f47.json"), "{w}");
    assert!(
        !w.contains(marker),
        "file contents never appear in warnings"
    );
    assert_eq!(spec.referenced_files, REF_FILES_MAX);
    assert_eq!(body_of(&spec, "POST /ok0").0, serde_json::json!("fine"));
    let max = paths.iter().position(|(p, _)| *p == "/max").unwrap();
    assert_eq!(
        body_of(&spec, &format!("POST /max{max}")).0,
        serde_json::json!(1)
    );
    // Unresolved references leave the body to the usual empty-body warning.
    let up = spec.build(&keys(&["POST /up1"]), 1);
    assert!(up.requests[0].body.text.is_empty());
    assert!(!up.requests[0].body.text.contains(marker));
}

#[test]
fn openapi_servers_and_variables_can_be_chosen() {
    let doc = r##"{
      "openapi": "3.0.3",
      "servers": [
        {"url": "https://{env}.example.com:{port}/v1", "description": "Cloud",
         "variables": {"env": {"default": "prod", "enum": ["prod", "staging"]},
                       "port": {"default": "8443"}}},
        {"url": "http://127.0.0.1:{port}", "variables": {"port": {"default": "8080", "enum": ["9090"]}}},
        {"url": "/relative"}
      ],
      "paths": {"/pets": {"get": {}}}
    }"##;
    let mut spec = load_openapi(doc.as_bytes(), None).unwrap();
    assert_eq!(spec.servers.len(), 3);
    let cloud = &spec.servers[0];
    assert_eq!(cloud.description, "Cloud");
    let vars: Vec<_> = cloud
        .variables
        .iter()
        .map(|v| (v.name.as_str(), v.default.as_str(), v.options.len()))
        .collect();
    assert_eq!(vars, [("env", "prod", 2), ("port", "8443", 0)]);
    // A default missing from the enum is offered first.
    assert_eq!(spec.servers[1].variables[0].options, ["8080", "9090"]);
    // The first server with its defaults is selected on load.
    assert_eq!(spec.base_url(), "https://prod.example.com:8443/v1");
    let all = keys(&["GET /pets"]);
    let url = |spec: &OpenApiSpec| spec.build(&all, 1).requests[0].url.clone();
    assert_eq!(url(&spec), "https://prod.example.com:8443/v1/pets");

    let mut choice = ServerChoice::defaults(&spec, 0);
    assert_eq!(choice.values, ["prod", "8443"]);
    choice.values[0] = "staging".into();
    spec.select_server(&choice, "").unwrap();
    assert_eq!(url(&spec), "https://staging.example.com:8443/v1/pets");
    let mut local = ServerChoice::defaults(&spec, 1);
    local.values[0] = "9090".into();
    assert_eq!(
        spec.select_server(&local, "").unwrap(),
        "http://127.0.0.1:9090"
    );
    assert_eq!(url(&spec), "http://127.0.0.1:9090/pets");
    // The Base URL field wins when it is not blank.
    spec.select_server(&local, " http://localhost:7000/ ")
        .unwrap();
    assert_eq!(url(&spec), "http://localhost:7000/pets");
    // A relative server cannot be used, and the base is cleared.
    let relative = ServerChoice::defaults(&spec, 2);
    assert!(
        spec.select_server(&relative, "")
            .unwrap_err()
            .contains("enter a base URL")
    );
    assert_eq!(spec.base_url(), "");
    for bad in ["http://user:pw@host", "ftp://host", "http://host/?a=1"] {
        assert!(spec.select_server(&local, bad).is_err(), "{bad}");
    }

    // Swagger 2.0: several schemes become a "scheme" choice.
    let swagger = r#"{"swagger": "2.0", "host": "api.example.com", "basePath": "/v2",
      "schemes": ["http", "https"], "paths": {"/pets": {"get": {}}}}"#;
    let mut spec = load_openapi(swagger.as_bytes(), None).unwrap();
    assert_eq!(spec.servers.len(), 1);
    let scheme = &spec.servers[0].variables[0];
    assert_eq!(
        (scheme.name.as_str(), scheme.options.clone()),
        ("scheme", vec!["http".to_owned(), "https".to_owned()])
    );
    assert_eq!(url(&spec), "http://api.example.com/v2/pets");
    let mut choice = ServerChoice::defaults(&spec, 0);
    choice.values[0] = "https".into();
    spec.select_server(&choice, "").unwrap();
    assert_eq!(url(&spec), "https://api.example.com/v2/pets");
    // A single scheme offers no choice.
    let one = swagger.replace(r#"["http", "https"]"#, r#"["https"]"#);
    let spec = load_openapi(one.as_bytes(), None).unwrap();
    assert!(spec.servers[0].variables[0].options.is_empty());
    assert_eq!(spec.base_url(), "https://api.example.com/v2");
    // Without a host or servers, loading works but a base URL is needed.
    let hostless = r#"{"swagger": "2.0", "paths": {"/a": {"get": {}}}}"#;
    let mut spec = load_openapi(hostless.as_bytes(), None).unwrap();
    assert_eq!(spec.base_url(), "");
    let none = ServerChoice::defaults(&spec, 0);
    assert!(
        spec.select_server(&none, "")
            .unwrap_err()
            .contains("no host")
    );
    spec.select_server(&none, "http://127.0.0.1:1").unwrap();
    assert_eq!(url_of(&spec, "GET /a"), "http://127.0.0.1:1/a");
}

fn url_of(spec: &OpenApiSpec, key: &str) -> String {
    spec.build(&keys(&[key]), 1).requests[0].url.clone()
}
