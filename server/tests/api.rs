use axum::{
    Router,
    body::Body,
    http::{HeaderMap, Method, Request, StatusCode, header},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD as B64};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::path::PathBuf;
use taskhit_server::{auth, backup, build_state, config::Config, router, themes};
use tower::ServiceExt;

fn app(tweak: impl FnOnce(&mut Config)) -> (Router, PathBuf) {
    let dir = std::env::temp_dir().join(format!("taskhit-test-{}", auth::random_hex(8)));
    let mut cfg = Config::default();
    cfg.paths.data_dir = dir.clone();
    cfg.paths.web_dir = dir.join("web");
    cfg.security.cookie_secure = false;
    tweak(&mut cfg);
    (router(build_state(cfg).expect("stato")), dir)
}

fn key(byte: u8) -> String { B64.encode([byte; 32]) }

async fn send(app: &Router, method: Method, uri: &str, body: Option<Value>, extra: &[(&str, &str)]) -> (StatusCode, HeaderMap, Value) {
    let mut req = Request::builder().method(method).uri(uri).header(header::HOST, "localhost");
    for (k, v) in extra {
        req = req.header(*k, *v);
    }
    let req = match body {
        Some(b) => req.header(header::CONTENT_TYPE, "application/json").body(Body::from(b.to_string())).unwrap(),
        None => req.body(Body::empty()).unwrap(),
    };
    let res = app.clone().oneshot(req).await.unwrap();
    let (status, headers) = (res.status(), res.headers().clone());
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, headers, json)
}

fn setup_body(username: &str, auth_byte: u8) -> Value {
    json!({
        "username": username,
        "display_name": "Lorenzo",
        "auth_key": key(auth_byte),
        "kdf_salt": B64.encode([9u8; 16]),
        "wrapped_key": B64.encode([5u8; 60]),
    })
}

fn cookie_of(headers: &HeaderMap) -> String {
    let raw = headers.get(header::SET_COOKIE).expect("Set-Cookie").to_str().unwrap();
    raw.split(';').next().unwrap().to_owned()
}

#[tokio::test]
async fn setup_login_me_logout() {
    let (app, _dir) = app(|_| {});
    let (s, _, body) = send(&app, Method::GET, "/api/setup/status", None, &[]).await;
    assert_eq!((s, body["needs_setup"].clone()), (StatusCode::OK, json!(true)));

    let (s, h, body) = send(&app, Method::POST, "/api/setup", Some(setup_body("Lorenzo", 1)), &[]).await;
    assert_eq!(s, StatusCode::OK, "{body}");
    assert_eq!(body["user"]["role"], "admin");
    assert_eq!(body["user"]["username"], "lorenzo", "il nome utente va normalizzato");
    let set_cookie = h.get(header::SET_COOKIE).unwrap().to_str().unwrap().to_owned();
    assert!(set_cookie.contains("HttpOnly") && set_cookie.contains("SameSite=Strict"), "{set_cookie}");

    // Il setup funziona una volta sola.
    let (s, _, _) = send(&app, Method::POST, "/api/setup", Some(setup_body("altro", 2)), &[]).await;
    assert_eq!(s, StatusCode::CONFLICT);
    let (_, _, body) = send(&app, Method::GET, "/api/setup/status", None, &[]).await;
    assert_eq!(body["needs_setup"], json!(false));

    // La sessione creata dal setup vale.
    let (s, _, me) = send(&app, Method::GET, "/api/me", None, &[("cookie", &cookie_of(&h))]).await;
    assert_eq!((s, me["username"].clone()), (StatusCode::OK, json!("lorenzo")));

    // Login: chiave sbagliata, poi giusta.
    let (s, _, _) = send(&app, Method::POST, "/api/auth/login", Some(json!({"username":"lorenzo","auth_key":key(2)})), &[]).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let (s, h2, body) = send(&app, Method::POST, "/api/auth/login", Some(json!({"username":"LORENZO","auth_key":key(1)})), &[]).await;
    assert_eq!(s, StatusCode::OK, "{body}");
    assert_eq!(body["wrapped_key"], B64.encode([5u8; 60]));
    assert_eq!(body["kdf_salt"], B64.encode([9u8; 16]));
    let cookie = cookie_of(&h2);

    let (s, _, _) = send(&app, Method::GET, "/api/me", None, &[]).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED, "senza cookie non si entra");
    let (s, _, _) = send(&app, Method::POST, "/api/auth/logout", Some(json!({})), &[("cookie", &cookie)]).await;
    assert_eq!(s, StatusCode::OK);
    let (s, _, _) = send(&app, Method::GET, "/api/me", None, &[("cookie", &cookie)]).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED, "dopo il logout il cookie non vale piu'");
}

#[tokio::test]
async fn setup_refused_through_tunnel_by_default() {
    let (app, _dir) = app(|_| {});
    let (s, _, _) = send(&app, Method::POST, "/api/setup", Some(setup_body("lorenzo", 1)), &[("cf-connecting-ip", "203.0.113.9")]).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _, _) = send(&app, Method::POST, "/api/setup", Some(setup_body("lorenzo", 1)), &[]).await;
    assert_eq!(s, StatusCode::OK, "dalla rete locale (senza intestazione Cloudflare) funziona");

    let (app2, _d2) = self::app(|c| c.security.allow_remote_setup = true);
    let (s, _, _) = send(&app2, Method::POST, "/api/setup", Some(setup_body("lorenzo", 1)), &[("cf-connecting-ip", "203.0.113.9")]).await;
    assert_eq!(s, StatusCode::OK, "con allow_remote_setup il tunnel e' ammesso");
}

#[tokio::test]
async fn prelogin_does_not_reveal_which_users_exist() {
    let (app, _dir) = app(|_| {});
    send(&app, Method::POST, "/api/setup", Some(setup_body("lorenzo", 1)), &[]).await;
    let (s, _, real) = send(&app, Method::POST, "/api/auth/prelogin", Some(json!({"username":"lorenzo"})), &[]).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(real["kdf_salt"], B64.encode([9u8; 16]));
    let (_, _, a) = send(&app, Method::POST, "/api/auth/prelogin", Some(json!({"username":"fantasma"})), &[]).await;
    let (_, _, b) = send(&app, Method::POST, "/api/auth/prelogin", Some(json!({"username":"fantasma"})), &[]).await;
    assert_eq!(a["kdf_salt"], b["kdf_salt"], "il salt finto deve essere stabile");
    assert_ne!(a["kdf_salt"], real["kdf_salt"]);
    assert_eq!(B64.decode(a["kdf_salt"].as_str().unwrap()).unwrap().len(), 16, "stessa forma di un salt vero");
    // Errore identico per utente inesistente e password sbagliata.
    let (s1, _, e1) = send(&app, Method::POST, "/api/auth/login", Some(json!({"username":"fantasma","auth_key":key(1)})), &[]).await;
    let (s2, _, e2) = send(&app, Method::POST, "/api/auth/login", Some(json!({"username":"lorenzo","auth_key":key(3)})), &[]).await;
    assert_eq!((s1, e1), (s2, e2));
}

#[tokio::test]
async fn login_attempts_are_rate_limited_per_user() {
    let (app, _dir) = app(|_| {});
    send(&app, Method::POST, "/api/setup", Some(setup_body("lorenzo", 1)), &[]).await;
    let mut last = StatusCode::OK;
    for _ in 0..6 {
        // IP diversi: il limite per nome utente scatta comunque.
        let (s, _, _) = send(&app, Method::POST, "/api/auth/login", Some(json!({"username":"lorenzo","auth_key":key(7)})), &[]).await;
        last = s;
    }
    assert_eq!(last, StatusCode::TOO_MANY_REQUESTS);
    // Anche la password giusta e' bloccata durante la pausa: un attaccante non puo' sfruttarla per indovinare.
    let (s, h, _) = send(&app, Method::POST, "/api/auth/login", Some(json!({"username":"lorenzo","auth_key":key(1)})), &[]).await;
    assert_eq!(s, StatusCode::TOO_MANY_REQUESTS);
    assert!(h.contains_key(header::RETRY_AFTER));
}

#[tokio::test]
async fn per_ip_limit_applies_to_the_whole_api() {
    let (app, _dir) = app(|c| c.security.api_requests_per_minute = 5);
    let mut statuses = vec![];
    for _ in 0..7 {
        let (s, _, _) = send(&app, Method::GET, "/api/health", None, &[("cf-connecting-ip", "198.51.100.4")]).await;
        statuses.push(s);
    }
    assert_eq!(&statuses[..5], &[StatusCode::OK; 5]);
    assert_eq!(statuses[6], StatusCode::TOO_MANY_REQUESTS);
    let (s, _, _) = send(&app, Method::GET, "/api/health", None, &[("cf-connecting-ip", "198.51.100.5")]).await;
    assert_eq!(s, StatusCode::OK, "un altro IP non e' toccato");
}

#[tokio::test]
async fn rejects_cross_origin_writes_and_oversized_bodies() {
    let (app, _dir) = app(|_| {});
    let body = json!({"username":"lorenzo","auth_key":key(1)});
    let (s, _, _) = send(&app, Method::POST, "/api/auth/login", Some(body.clone()), &[("origin", "https://evil.example")]).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _, _) = send(&app, Method::POST, "/api/auth/login", Some(body.clone()), &[("sec-fetch-site", "cross-site")]).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _, _) = send(&app, Method::POST, "/api/auth/login", Some(body), &[("origin", "http://localhost")]).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED, "stessa origine: arriva al login");

    let big = json!({"username":"lorenzo","auth_key":"x".repeat(300_000)});
    let (s, _, _) = send(&app, Method::POST, "/api/auth/login", Some(big), &[]).await;
    assert_eq!(s, StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn public_url_pins_the_allowed_origin() {
    let (app, _dir) = app(|c| c.server.public_url = Some("https://taskhit.example".into()));
    let body = json!({"username":"lorenzo","auth_key":key(1)});
    let (s, _, _) = send(&app, Method::POST, "/api/auth/login", Some(body.clone()), &[("origin", "http://localhost")]).await;
    assert_eq!(s, StatusCode::FORBIDDEN, "con public_url vale solo quella origine");
    let (s, _, _) = send(&app, Method::POST, "/api/auth/login", Some(body), &[("origin", "https://taskhit.example")]).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn api_hardening_headers_and_unknown_routes() {
    let (app, _dir) = app(|c| c.security.cookie_secure = true);
    let (s, h, body) = send(&app, Method::GET, "/api/health", None, &[]).await;
    assert_eq!((s, body["status"].clone()), (StatusCode::OK, json!("ok")));
    let get = |n: &str| h.get(n).map(|v| v.to_str().unwrap().to_owned()).unwrap_or_default();
    assert!(get("content-security-policy").contains("default-src 'self'"));
    assert!(get("content-security-policy").contains("frame-ancestors 'none'"));
    // Argon2id nel browser e' WebAssembly: va consentito, senza aprire all'eval di JavaScript.
    assert!(get("content-security-policy").contains("script-src 'self' 'wasm-unsafe-eval';"));
    assert!(!get("content-security-policy").contains(" 'unsafe-eval'"));
    assert_eq!(get("x-content-type-options"), "nosniff");
    assert_eq!(get("cache-control"), "no-store");
    assert!(get("strict-transport-security").starts_with("max-age="));

    let (s, _, body) = send(&app, Method::GET, "/api/inesistente", None, &[]).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    assert!(body["error"].is_string(), "le API sconosciute non finiscono nell'interfaccia web");

    // Con HTTPS il cookie ha il prefisso __Host- e l'attributo Secure.
    let (_, h, _) = send(&app, Method::POST, "/api/setup", Some(setup_body("lorenzo", 1)), &[]).await;
    let c = h.get(header::SET_COOKIE).unwrap().to_str().unwrap();
    assert!(c.starts_with("__Host-th_session=") && c.contains("; Secure"), "{c}");
}

#[tokio::test]
async fn setup_rejects_malformed_keys() {
    let (app, _dir) = app(|_| {});
    for (field, bad) in [("auth_key", json!("corta")), ("kdf_salt", json!(B64.encode([1u8; 8]))), ("wrapped_key", json!("***")), ("username", json!("a b"))] {
        let mut body = setup_body("lorenzo", 1);
        body[field] = bad;
        let (s, _, _) = send(&app, Method::POST, "/api/setup", Some(body), &[]).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "campo {field}");
    }
    let (_, _, st) = send(&app, Method::GET, "/api/setup/status", None, &[]).await;
    assert_eq!(st["needs_setup"], json!(true), "nessun utente creato dai tentativi falliti");
}

#[tokio::test]
async fn themes_are_seeded_validated_and_filtered() {
    let (app, dir) = app(|_| {});
    let (s, _, list) = send(&app, Method::GET, "/api/themes", None, &[]).await;
    assert_eq!(s, StatusCode::OK);
    let ids: Vec<_> = list["themes"].as_array().unwrap().iter().map(|t| t["id"].as_str().unwrap().to_owned()).collect();
    assert_eq!(ids, ["carta", "lavagna", "notte", "quaderno"]);
    assert!(list["invalid"].as_array().unwrap().is_empty());

    // Un tema nuovo = un solo file nella cartella.
    let themes_dir = dir.join("themes");
    let good = std::fs::read_to_string(themes_dir.join("carta.json")).unwrap().replace("Carta", "Sabbia").replace("#F1F0EB", "#EFE6D2").replace("#6B6B73", "#5E5E66");
    std::fs::write(themes_dir.join("sabbia.json"), good).unwrap();
    // Contrasto illeggibile, nome file non valido, font con caratteri pericolosi.
    let low = std::fs::read_to_string(themes_dir.join("carta.json")).unwrap().replace("#17171B", "#E0E0DA");
    std::fs::write(themes_dir.join("illeggibile.json"), low).unwrap();
    std::fs::write(themes_dir.join("Nome Brutto.json"), "{}").unwrap();
    let evil = std::fs::read_to_string(themes_dir.join("carta.json")).unwrap().replace("Geist Mono", "x\"; } body { display:none");
    std::fs::write(themes_dir.join("font-strano.json"), evil).unwrap();

    let (_, _, list) = send(&app, Method::GET, "/api/themes", None, &[]).await;
    let ids: Vec<_> = list["themes"].as_array().unwrap().iter().map(|t| t["id"].as_str().unwrap().to_owned()).collect();
    assert!(ids.contains(&"sabbia".to_owned()), "{ids:?}");
    assert!(!ids.contains(&"illeggibile".to_owned()) && !ids.contains(&"font-strano".to_owned()));
    let invalid: Vec<_> = list["invalid"].as_array().unwrap().iter().map(|i| i["file"].as_str().unwrap().to_owned()).collect();
    assert_eq!(invalid.len(), 3, "{invalid:?}");
}

#[test]
fn contrast_matches_wcag_reference_values() {
    assert!((themes::contrast("#000000", "#FFFFFF").unwrap() - 21.0).abs() < 0.01);
    assert!((themes::contrast("#777777", "#FFFFFF").unwrap() - 4.48).abs() < 0.05);
    assert!(themes::contrast("rosso", "#FFFFFF").is_none());
}

#[tokio::test]
async fn data_survives_restart_and_migrations_are_idempotent() {
    let dir = std::env::temp_dir().join(format!("taskhit-test-{}", auth::random_hex(8)));
    let make = || {
        let mut cfg = Config::default();
        cfg.paths.data_dir = dir.clone();
        cfg.security.cookie_secure = false;
        router(build_state(cfg).unwrap())
    };
    let first = make();
    send(&first, Method::POST, "/api/setup", Some(setup_body("lorenzo", 1)), &[]).await;
    drop(first);
    let second = make(); // secondo avvio sulla stessa cartella: stesse chiavi, stesso database
    let (_, _, st) = send(&second, Method::GET, "/api/setup/status", None, &[]).await;
    assert_eq!(st["needs_setup"], json!(false));
    let (s, _, _) = send(&second, Method::POST, "/api/auth/login", Some(json!({"username":"lorenzo","auth_key":key(1)})), &[]).await;
    assert_eq!(s, StatusCode::OK);
}

#[tokio::test]
async fn web_fallback_serves_app_routes_but_404s_missing_files() {
    let (app, dir) = app(|_| {});
    let web = dir.join("web");
    std::fs::create_dir_all(web.join("assets")).unwrap();
    std::fs::write(web.join("index.html"), "<!doctype html><title>TaskHit</title>").unwrap();
    std::fs::write(web.join("assets/app.js"), "console.log(1)").unwrap();

    let get = |uri: &'static str| {
        let app = app.clone();
        async move {
            let res = app.oneshot(Request::builder().uri(uri).header(header::HOST, "localhost").body(Body::empty()).unwrap()).await.unwrap();
            let (s, h) = (res.status(), res.headers().clone());
            let body = String::from_utf8_lossy(&res.into_body().collect().await.unwrap().to_bytes()).into_owned();
            (s, h, body)
        }
    };
    let (s, _, b) = get("/").await;
    assert!(s == StatusCode::OK && b.contains("TaskHit"));
    let (s, h, b) = get("/calendario/2026-11").await;
    assert!(s == StatusCode::OK && b.contains("TaskHit"), "le rotte dell'app ricadono su index.html");
    assert!(h[header::CONTENT_TYPE].to_str().unwrap().starts_with("text/html"));
    let (s, _, b) = get("/assets/app.js").await;
    assert!(s == StatusCode::OK && b.contains("console.log"));
    let (s, _, _) = get("/assets/mancante.js").await;
    assert_eq!(s, StatusCode::NOT_FOUND, "un asset mancante non deve diventare HTML");
    let (s, _, b) = get("/assets/%2e%2e/%2e%2e/etc/passwd").await;
    assert!(!b.contains("root:"), "nessuna uscita dalla cartella web ({s})");
}

// --- Calendario, ruoli, proposte, inviti ---

/// Admin + una classe + un utente per ogni ruolo indicato; restituisce i cookie.
struct Classe {
    app: Router,
    admin: String,
    group: String,
}

impl Classe {
    async fn nuova() -> Self {
        let (app, _dir) = app(|c| {
            c.security.api_requests_per_minute = 10_000;
            c.security.login_attempts_per_minute_ip = 1_000;
        });
        let (_, h, _) = send(&app, Method::POST, "/api/setup", Some(setup_body("admin", 1)), &[]).await;
        let admin = cookie_of(&h);
        let (s, _, g) = send(&app, Method::POST, "/api/groups", Some(json!({"name":"4B","kind":"classe"})), &[("cookie", &admin)]).await;
        assert_eq!(s, StatusCode::OK, "{g}");
        Self { app, admin, group: g["id"].as_str().unwrap().to_owned() }
    }

    async fn get(&self, uri: &str, cookie: &str) -> (StatusCode, Value) {
        let (s, _, b) = send(&self.app, Method::GET, uri, None, &[("cookie", cookie)]).await;
        (s, b)
    }

    async fn post(&self, uri: &str, body: Value, cookie: &str) -> (StatusCode, Value) {
        let (s, _, b) = send(&self.app, Method::POST, uri, Some(body), &[("cookie", cookie)]).await;
        (s, b)
    }

    /// Entra con un codice invito, viene accettato, riceve il ruolo. Restituisce (id, cookie).
    async fn iscrivi(&self, username: &str, role: &str, byte: u8) -> (String, String) {
        let (_, inv) = self.post("/api/invites", json!({"group_id": self.group}), &self.admin).await;
        let mut body = setup_body(username, byte);
        body["code"] = inv["code"].clone();
        let (s, _, b) = send(&self.app, Method::POST, "/api/join", Some(body), &[]).await;
        assert_eq!(s, StatusCode::OK, "{b}");
        let (_, list) = self.get("/api/users", &self.admin).await;
        let id = list["pending"][0]["id"].as_str().unwrap().to_owned();
        self.post(&format!("/api/users/{id}/accept"), json!({}), &self.admin).await;
        let (s, _, b) = send(&self.app, Method::PATCH, &format!("/api/users/{id}"), Some(json!({"role": role})), &[("cookie", &self.admin)]).await;
        assert_eq!(s, StatusCode::OK, "{b}");
        let (s, h, b) = send(&self.app, Method::POST, "/api/auth/login", Some(json!({"username":username,"auth_key":key(byte)})), &[]).await;
        assert_eq!(s, StatusCode::OK, "{b}");
        (id, cookie_of(&h))
    }

    fn evento(&self, title: &str) -> Value {
        json!({"visibility":"public","data":{"group_id": self.group, "title": title, "starts_at": 1_795_500_000, "ends_at": 1_795_506_600}})
    }

    async fn eventi(&self, cookie: &str) -> Vec<Value> {
        let (s, b) = self.get("/api/events", cookie).await;
        assert_eq!(s, StatusCode::OK, "{b}");
        b["events"].as_array().unwrap().clone()
    }
}

#[tokio::test]
async fn invites_require_admin_approval() {
    let c = Classe::nuova().await;
    let (_, inv) = c.post("/api/invites", json!({"group_id": c.group}), &c.admin).await;
    let code = inv["code"].as_str().unwrap().to_owned();

    let mut body = setup_body("anna", 4);
    body["code"] = json!("AAAAAAAAA");
    let (s, _, _) = send(&c.app, Method::POST, "/api/join", Some(body.clone()), &[]).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "codice inventato");
    // Il codice si accetta anche in minuscolo e con i trattini.
    body["code"] = json!(format!("{}-{}-{}", &code[..3], &code[3..6], &code[6..]).to_lowercase());
    let (s, _, b) = send(&c.app, Method::POST, "/api/join", Some(body.clone()), &[]).await;
    assert_eq!(s, StatusCode::OK, "{b}");

    let login = json!({"username":"anna","auth_key":key(4)});
    let (s, h, b) = send(&c.app, Method::POST, "/api/auth/login", Some(login.clone()), &[]).await;
    assert_eq!(s, StatusCode::FORBIDDEN, "{b}");
    assert!(!h.contains_key(header::SET_COOKIE), "nessuna sessione prima dell'approvazione");

    let (_, list) = c.get("/api/users", &c.admin).await;
    assert_eq!(list["users"].as_array().unwrap().len(), 1);
    let id = list["pending"][0]["id"].as_str().unwrap().to_owned();
    let (s, _) = c.post(&format!("/api/users/{id}/accept"), json!({}), &c.admin).await;
    assert_eq!(s, StatusCode::OK);
    let (s, h, b) = send(&c.app, Method::POST, "/api/auth/login", Some(login), &[]).await;
    assert_eq!((s, b["user"]["role"].clone()), (StatusCode::OK, json!("alunno")), "chi entra con un codice e' alunno");
    let anna = cookie_of(&h);
    let (_, meta) = c.get("/api/meta", &anna).await;
    assert_eq!(meta["groups"][0]["name"], "4B");
    assert_eq!(meta["tags"].as_array().unwrap().len(), 7, "materie di partenza");
    assert_eq!(meta["labels"].as_array().unwrap().len(), 8);

    // Un codice revocato non vale piu'.
    let (_, invs) = c.get("/api/invites", &c.admin).await;
    let iid = invs["invites"][0]["id"].as_str().unwrap().to_owned();
    c.post(&format!("/api/invites/{iid}/revoke"), json!({}), &c.admin).await;
    body["username"] = json!("bruno");
    let (s, _, _) = send(&c.app, Method::POST, "/api/join", Some(body), &[]).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);

    // Le operazioni di amministrazione sono solo dell'admin.
    for (uri, body) in [("/api/groups", json!({"name":"x","kind":"gruppo"})), ("/api/invites", json!({"group_id": c.group})), ("/api/labels", json!({"name":"x"}))] {
        let (s, _) = c.post(uri, body, &anna).await;
        assert_eq!(s, StatusCode::FORBIDDEN, "{uri}");
    }
    for uri in ["/api/users", "/api/invites", "/api/admin/status"] {
        assert_eq!(c.get(uri, &anna).await.0, StatusCode::FORBIDDEN, "{uri}");
    }
}

#[tokio::test]
async fn admin_cannot_demote_or_disable_themselves() {
    let c = Classe::nuova().await;
    let (_, me) = c.get("/api/me", &c.admin).await;
    let uri = format!("/api/users/{}", me["id"].as_str().unwrap());
    for body in [json!({"role":"alunno"}), json!({"disabled":true})] {
        let (s, _, _) = send(&c.app, Method::PATCH, &uri, Some(body), &[("cookie", &c.admin)]).await;
        assert_eq!(s, StatusCode::FORBIDDEN);
    }
}

#[tokio::test]
async fn public_events_follow_roles_and_fill_the_inbox() {
    let c = Classe::nuova().await;
    let (_, rap) = c.iscrivi("giulia", "rappresentante", 2).await;
    let (_, alunno) = c.iscrivi("anna", "alunno", 3).await;

    let (s, b) = c.post("/api/events", c.evento("Limiti"), &alunno).await;
    assert_eq!(s, StatusCode::FORBIDDEN, "{b}");

    let (s, b) = c.post("/api/events", c.evento("Limiti"), &rap).await;
    assert_eq!((s, b["status"].clone()), (StatusCode::OK, json!("published")));
    let id = b["id"].as_str().unwrap().to_owned();
    let seen = c.eventi(&alunno).await;
    assert_eq!(seen[0]["data"]["title"], "Limiti");
    assert_eq!(seen[0]["created_by"]["role"], "rappresentante");

    let (_, inbox) = c.get("/api/inbox", &alunno).await;
    assert_eq!((inbox["unread"].clone(), inbox["items"][0]["kind"].clone()), (json!(1), json!("added")));
    let (_, own) = c.get("/api/inbox", &rap).await;
    assert_eq!(own["unread"], 0, "chi scrive non si autonotifica");

    // Revisione vecchia: conflitto, niente sovrascritture silenziose.
    let mut data = seen[0]["data"].clone();
    data["title"] = json!("Limiti e continuita'");
    let uri = format!("/api/events/{id}");
    let (s, _, b) = send(&c.app, Method::PUT, &uri, Some(json!({"rev":1,"data":data})), &[("cookie", &rap)]).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let (s, _, _) = send(&c.app, Method::PUT, &uri, Some(json!({"rev":1,"data":data})), &[("cookie", &rap)]).await;
    assert_eq!(s, StatusCode::CONFLICT);
    let (s, _, _) = send(&c.app, Method::PUT, &uri, Some(json!({"rev":2,"data":data})), &[("cookie", &alunno)]).await;
    assert_eq!(s, StatusCode::FORBIDDEN);

    let (_, inbox) = c.get("/api/inbox", &alunno).await;
    assert_eq!(inbox["items"][0]["payload"]["changes"]["title"], json!(["Limiti", "Limiti e continuita'"]));
    c.post("/api/inbox/read", json!({}), &alunno).await;
    assert_eq!(c.get("/api/inbox", &alunno).await.1["unread"], 0);

    let (s, _) = c.post(&format!("{uri}/delete"), json!({"rev":2}), &alunno).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = c.post(&format!("{uri}/delete"), json!({"rev":2}), &rap).await;
    assert_eq!(s, StatusCode::OK);
    assert!(c.eventi(&alunno).await.is_empty());
    assert_eq!(c.get("/api/inbox", &alunno).await.1["items"][0]["kind"], "deleted");
}

#[tokio::test]
async fn nerd_changes_wait_for_approval() {
    let c = Classe::nuova().await;
    let (_, rap) = c.iscrivi("giulia", "rappresentante", 2).await;
    let (_, nerd) = c.iscrivi("luca", "nerd", 3).await;
    let (_, alunno) = c.iscrivi("anna", "alunno", 4).await;

    let (s, b) = c.post("/api/events", c.evento("VLAN"), &nerd).await;
    assert_eq!((s, b["status"].clone()), (StatusCode::OK, json!("proposed")));
    let pid = b["id"].as_str().unwrap().to_owned();
    assert!(c.eventi(&alunno).await.is_empty(), "una proposta non e' visibile alla classe");
    assert_eq!(c.get("/api/inbox", &rap).await.1["to_approve"], 1);

    let decide = format!("/api/proposals/{pid}/decide");
    for who in [&nerd, &alunno] {
        let (s, _) = c.post(&decide, json!({"approve":true}), who).await;
        assert_eq!(s, StatusCode::FORBIDDEN, "nessuno approva senza averne il ruolo");
    }
    let (s, b) = c.post(&decide, json!({"approve":true}), &rap).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let (s, _) = c.post(&decide, json!({"approve":false}), &rap).await;
    assert_eq!(s, StatusCode::CONFLICT, "si decide una volta sola");

    let ev = c.eventi(&alunno).await.remove(0);
    assert_eq!((ev["created_by"]["name"].clone(), ev["approved_by"].is_string()), (json!("Lorenzo"), true));
    assert_eq!(c.get("/api/inbox", &nerd).await.1["items"][0]["payload"]["outcome"], "approved");
    let id = ev["id"].as_str().unwrap().to_owned();

    // Una proposta superata da una modifica piu' recente non si puo' approvare.
    let mut data = ev["data"].clone();
    data["title"] = json!("VLAN e routing");
    let (s, _, b) = send(&c.app, Method::PUT, &format!("/api/events/{id}"), Some(json!({"rev":1,"data":data,"note":"spostata"})), &[("cookie", &nerd)]).await;
    assert_eq!((s, b["status"].clone()), (StatusCode::OK, json!("proposed")));
    data["title"] = json!("Routing");
    send(&c.app, Method::PUT, &format!("/api/events/{id}"), Some(json!({"rev":1,"data":data})), &[("cookie", &rap)]).await;
    let (_, list) = c.get("/api/proposals", &rap).await;
    assert_eq!(list["proposals"][0]["stale"], true);
    let stale = format!("/api/proposals/{}/decide", list["proposals"][0]["id"].as_str().unwrap());
    assert_eq!(c.post(&stale, json!({"approve":true}), &rap).await.0, StatusCode::CONFLICT);
    assert_eq!(c.post(&stale, json!({"approve":false}), &rap).await.0, StatusCode::OK);

    // Eliminazione proposta e approvata.
    let (s, b) = c.post(&format!("/api/events/{id}/delete"), json!({"rev":2,"note":"annullata"}), &nerd).await;
    assert_eq!((s, b["status"].clone()), (StatusCode::OK, json!("proposed")));
    assert_eq!(c.eventi(&alunno).await.len(), 1);
    let (_, list) = c.get("/api/proposals", &rap).await;
    let del = format!("/api/proposals/{}/decide", list["proposals"][0]["id"].as_str().unwrap());
    assert_eq!(c.post(&del, json!({"approve":true}), &rap).await.0, StatusCode::OK);
    assert!(c.eventi(&alunno).await.is_empty());
}

#[tokio::test]
async fn private_events_are_opaque_and_owner_only() {
    let c = Classe::nuova().await;
    let (_, anna) = c.iscrivi("anna", "alunno", 3).await;
    let id = "0123456789abcdef0123456789abcdef";
    let blob = B64.encode([7u8; 80]);

    let (s, _) = c.post("/api/events", json!({"visibility":"private","id":id,"data":{"group_id":c.group,"title":"x","starts_at":1}}), &anna).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "un privato non puo' portare campi in chiaro");
    let (s, b) = c.post("/api/events", json!({"visibility":"private","id":id,"payload":blob}), &anna).await;
    assert_eq!(s, StatusCode::OK, "{b}");

    assert_eq!(c.eventi(&anna).await[0]["payload"], json!(blob));
    assert!(c.eventi(&c.admin).await.is_empty(), "nemmeno l'admin lo riceve");
    let uri = format!("/api/events/{id}");
    assert_eq!(c.get(&format!("{uri}/history"), &c.admin).await.0, StatusCode::FORBIDDEN);
    let (s, _) = c.post(&format!("{uri}/delete"), json!({"rev":1}), &c.admin).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    assert_eq!(c.get("/api/inbox", &c.admin).await.1["items"].as_array().unwrap().len(), 0, "nessuna notifica");

    let (s, _) = c.post(&format!("{uri}/delete"), json!({"rev":1}), &anna).await;
    assert_eq!(s, StatusCode::OK);
    assert!(c.eventi(&anna).await.is_empty());
}

#[tokio::test]
async fn groups_isolate_public_events() {
    let c = Classe::nuova().await;
    let (rid, rap) = c.iscrivi("giulia", "rappresentante", 2).await;
    let (_, g) = c.post("/api/groups", json!({"name":"Corso reti","kind":"gruppo"}), &c.admin).await;
    let altro = g["id"].as_str().unwrap().to_owned();
    let body = json!({"visibility":"public","data":{"group_id":altro,"title":"Modulo 3","starts_at":1_795_500_000}});
    assert_eq!(c.post("/api/events", body.clone(), &rap).await.0, StatusCode::FORBIDDEN);
    let (mid, marco) = c.iscrivi("marco", "rappresentante", 3).await;
    send(&c.app, Method::PATCH, &format!("/api/users/{mid}"), Some(json!({"group_ids":[altro]})), &[("cookie", &c.admin)]).await;
    let (s, made) = c.post("/api/events", body, &marco).await;
    assert_eq!(s, StatusCode::OK, "{made}");
    assert!(c.eventi(&rap).await.is_empty(), "chi non fa parte del gruppo non vede l'evento");
    let uri = format!("/api/events/{}/history", made["id"].as_str().unwrap());
    assert_eq!(c.get(&uri, &rap).await.0, StatusCode::NOT_FOUND);

    send(&c.app, Method::PATCH, &format!("/api/users/{rid}"), Some(json!({"group_ids":[c.group, altro]})), &[("cookie", &c.admin)]).await;
    assert_eq!(c.eventi(&rap).await.len(), 1);
    // Un account disattivato perde subito la sessione.
    send(&c.app, Method::PATCH, &format!("/api/users/{rid}"), Some(json!({"disabled":true})), &[("cookie", &c.admin)]).await;
    assert_eq!(c.get("/api/events", &rap).await.0, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn event_fields_are_validated() {
    let c = Classe::nuova().await;
    let (_, rap) = c.iscrivi("giulia", "rappresentante", 2).await;
    let ok = c.evento("Prova")["data"].clone();
    for (field, bad) in [
        ("title", json!("   ")),
        ("ends_at", json!(1_795_400_000)),
        ("group_id", json!("0123456789abcdef0123456789abcdef")),
        ("tag_id", json!("inesistente")),
        ("sub_kind", json!("teorica")),
        ("alerts", json!(["domani"])),
    ] {
        let mut data = ok.clone();
        data[field] = bad;
        let (s, b) = c.post("/api/events", json!({"visibility":"public","data":data}), &rap).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "campo {field}: {b}");
    }
    let (s, _) = c.post("/api/tags", json!({"name":"Fisica","color":"red"}), &rap).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn the_admin_runs_the_server_and_stays_out_of_the_class() {
    let c = Classe::nuova().await;
    let (_, me) = c.get("/api/me", &c.admin).await;
    let admin_id = me["id"].as_str().unwrap().to_owned();
    let (rid, rap) = c.iscrivi("giulia", "rappresentante", 2).await;
    let (_, nerd) = c.iscrivi("luca", "nerd", 3).await;

    // Non compare tra i membri, nemmeno del gruppo che ha creato.
    let (_, meta) = c.get("/api/meta", &rap).await;
    let members = meta["groups"][0]["members"].as_array().unwrap();
    assert!(members.len() == 2 && members.iter().all(|m| m["role"] != "admin"), "{members:?}");
    let (_, meta) = c.get("/api/meta", &c.admin).await;
    assert_eq!(meta["groups"][0]["mine"], false, "vede il gruppo per gestirlo, senza farne parte");

    // Niente eventi, materie, tipi o proposte.
    let private = json!({"visibility":"private","id":"0123456789abcdef0123456789abcdef","payload":B64.encode([7u8; 80])});
    for (uri, body) in [
        ("/api/events", c.evento("No")),
        ("/api/events", private),
        ("/api/tags", json!({"name":"Fisica","color":"#112233"})),
        ("/api/labels", json!({"name":"Gita"})),
    ] {
        let (s, b) = c.post(uri, body, &c.admin).await;
        assert_eq!(s, StatusCode::FORBIDDEN, "{uri}: {b}");
    }
    let (_, made) = c.post("/api/events", c.evento("Compito"), &rap).await;
    assert!(c.eventi(&c.admin).await.is_empty(), "il calendario non e' affare dell'admin");
    let (s, _) = c.post(&format!("/api/events/{}/delete", made["id"].as_str().unwrap()), json!({"rev":1}), &c.admin).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (_, proposal) = c.post("/api/events", c.evento("Gita"), &nerd).await;
    let decide = format!("/api/proposals/{}/decide", proposal["id"].as_str().unwrap());
    assert_eq!(c.post(&decide, json!({"approve":true}), &c.admin).await.0, StatusCode::FORBIDDEN);
    assert!(c.get("/api/proposals", &c.admin).await.1["proposals"].as_array().unwrap().is_empty());
    assert!(c.get("/api/inbox", &c.admin).await.1["items"].as_array().unwrap().is_empty(), "nessuna notifica sugli eventi");

    // Non puo' essere l'alunno di un evento.
    let mut body = c.evento("Interrogazione");
    body["data"]["subject_id"] = json!(admin_id);
    assert_eq!(c.post("/api/events", body, &rap).await.0, StatusCode::BAD_REQUEST);

    // Il rappresentante cura i contenuti della classe.
    for (uri, body) in [("/api/tags", json!({"name":"Fisica","color":"#112233"})), ("/api/labels", json!({"name":"Gita"}))] {
        assert_eq!(c.post(uri, body, &rap).await.0, StatusCode::OK, "{uri}");
    }
    let plugin = json!({"scope":"instance","enabled":false});
    let (s, _, _) = send(&c.app, Method::PUT, "/api/plugins/grafico", Some(plugin.clone()), &[("cookie", &nerd)]).await;
    assert_eq!(s, StatusCode::FORBIDDEN, "i plugin per tutti: solo admin e rappresentanti");
    let (s, _, _) = send(&c.app, Method::PUT, "/api/plugins/grafico", Some(plugin.clone()), &[("cookie", &rap)]).await;
    assert_eq!(s, StatusCode::OK);
    let (s, _, _) = send(&c.app, Method::PUT, "/api/plugins/grafico", Some(plugin), &[("cookie", &c.admin)]).await;
    assert_eq!(s, StatusCode::OK);

    // Chi diventa admin esce dalla classe, anche se la richiesta indica ancora i gruppi.
    let (s, _, _) = send(&c.app, Method::PATCH, &format!("/api/users/{rid}"), Some(json!({"role":"admin","group_ids":[c.group]})), &[("cookie", &c.admin)]).await;
    assert_eq!(s, StatusCode::OK);
    let (_, list) = c.get("/api/users", &c.admin).await;
    let giulia = list["users"].as_array().unwrap().iter().find(|u| u["id"] == rid.as_str()).unwrap().clone();
    assert_eq!(giulia["group_ids"], json!([]));
    assert!(c.eventi(&rap).await.is_empty());
}

#[tokio::test]
async fn representatives_manage_invites_and_groups_of_their_own_class_only() {
    let c = Classe::nuova().await;
    let (rid, rap) = c.iscrivi("giulia", "rappresentante", 2).await;
    let (aid, anna) = c.iscrivi("anna", "alunno", 3).await;
    let (_, altra) = c.post("/api/groups", json!({"name":"5C","kind":"classe"}), &c.admin).await;
    let altra = altra["id"].as_str().unwrap().to_owned();
    let (_, fuori) = c.post("/api/invites", json!({"group_id": altra}), &c.admin).await;
    let mut estraneo = setup_body("estraneo", 8);
    estraneo["code"] = fuori["code"].clone();
    send(&c.app, Method::POST, "/api/join", Some(estraneo), &[]).await;
    let (_, list) = c.get("/api/users", &c.admin).await;
    let estraneo = list["pending"][0]["id"].as_str().unwrap().to_owned();

    // Inviti: solo per la propria classe; richieste di altre classi invisibili.
    assert_eq!(c.post("/api/invites", json!({"group_id": altra}), &rap).await.0, StatusCode::FORBIDDEN);
    let (s, inv) = c.post("/api/invites", json!({"group_id": c.group}), &rap).await;
    assert_eq!(s, StatusCode::OK, "{inv}");
    let (_, invs) = c.get("/api/invites", &rap).await;
    assert!(invs["invites"].as_array().unwrap().iter().all(|i| i["group_id"] == c.group.as_str()), "{invs}");
    let (_, all) = c.get("/api/invites", &c.admin).await;
    let foreign = all["invites"].as_array().unwrap().iter().find(|i| i["group_id"] == altra.as_str()).unwrap()["id"].as_str().unwrap().to_owned();
    assert_eq!(c.post(&format!("/api/invites/{foreign}/revoke"), json!({}), &rap).await.0, StatusCode::NOT_FOUND);
    for action in ["accept", "reject"] {
        assert_eq!(c.post(&format!("/api/users/{estraneo}/{action}"), json!({}), &rap).await.0, StatusCode::NOT_FOUND, "{action}");
    }
    assert!(c.get("/api/users", &rap).await.1["pending"].as_array().unwrap().is_empty());

    // Accetta chi chiede di entrare nella sua classe.
    let mut body = setup_body("bruno", 4);
    body["code"] = inv["code"].clone();
    send(&c.app, Method::POST, "/api/join", Some(body), &[]).await;
    assert_eq!(c.get("/api/meta", &rap).await.1["pending"], 1);
    let (_, list) = c.get("/api/users", &rap).await;
    assert!(list["users"].as_array().unwrap().is_empty(), "la gestione delle persone resta all'admin");
    let bruno = list["pending"][0]["id"].as_str().unwrap().to_owned();
    assert_eq!(c.post(&format!("/api/users/{bruno}/accept"), json!({}), &rap).await.0, StatusCode::OK);
    let (s, _, _) = send(&c.app, Method::PATCH, &format!("/api/users/{aid}"), Some(json!({"role":"nerd"})), &[("cookie", &rap)]).await;
    assert_eq!(s, StatusCode::FORBIDDEN, "i ruoli li cambia l'admin");

    // Gruppi: solo dentro la propria classe.
    for body in [
        json!({"name":"x","kind":"classe"}),
        json!({"name":"x","kind":"gruppo"}),
        json!({"name":"x","kind":"gruppo","parent_id":altra}),
    ] {
        assert_eq!(c.post("/api/groups", body.clone(), &rap).await.0, StatusCode::FORBIDDEN, "{body}");
    }
    let nuovo = json!({"name":"Laboratorio","kind":"gruppo","parent_id":c.group});
    assert_eq!(c.post("/api/groups", nuovo.clone(), &anna).await.0, StatusCode::FORBIDDEN);
    let (s, g) = c.post("/api/groups", nuovo, &rap).await;
    assert_eq!(s, StatusCode::OK, "{g}");
    let lab = g["id"].as_str().unwrap().to_owned();
    let uri = format!("/api/groups/{lab}/members");
    let put = |uri: String, ids: Value, cookie: String| {
        let app = c.app.clone();
        async move { send(&app, Method::PUT, &uri, Some(json!({"user_ids": ids})), &[("cookie", &cookie)]).await.0 }
    };
    assert_eq!(put(uri.clone(), json!([rid, estraneo]), rap.clone()).await, StatusCode::BAD_REQUEST, "solo compagni di classe");
    assert_eq!(put(uri.clone(), json!([rid, aid]), anna.clone()).await, StatusCode::FORBIDDEN);
    assert_eq!(put(uri.clone(), json!([rid, aid]), rap.clone()).await, StatusCode::OK);
    assert_eq!(put(format!("/api/groups/{}/members", c.group), json!([rid]), rap.clone()).await, StatusCode::FORBIDDEN, "la classe la gestisce l'admin");
    assert_eq!(put(format!("/api/groups/{altra}/members"), json!([]), rap.clone()).await, StatusCode::NOT_FOUND);
    let (_, meta) = c.get("/api/meta", &anna).await;
    assert_eq!(meta["groups"].as_array().unwrap().len(), 2, "{meta}");

    // Chi esce dalla classe esce anche dai suoi gruppi.
    send(&c.app, Method::PATCH, &format!("/api/users/{aid}"), Some(json!({"group_ids":[altra, lab]})), &[("cookie", &c.admin)]).await;
    let (_, meta) = c.get("/api/meta", &anna).await;
    let names: Vec<_> = meta["groups"].as_array().unwrap().iter().map(|g| g["name"].clone()).collect();
    assert_eq!(names, [json!("5C")]);
}

// --- Backup e ripristino ---------------------------------------------------------------------

async fn send_bytes(app: &Router, uri: &str, bytes: Vec<u8>, extra: &[(&str, &str)]) -> (StatusCode, Value) {
    let mut req = Request::builder().method(Method::POST).uri(uri).header(header::HOST, "localhost").header(header::CONTENT_TYPE, "application/octet-stream");
    for (k, v) in extra {
        req = req.header(*k, *v);
    }
    let res = app.clone().oneshot(req.body(Body::from(bytes)).unwrap()).await.unwrap();
    let status = res.status();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
}

async fn download(app: &Router, name: &str, cookie: &str) -> (StatusCode, Vec<u8>) {
    let req = Request::builder().uri(format!("/api/backups/{name}")).header(header::HOST, "localhost").header("cookie", cookie).body(Body::empty()).unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    (status, res.into_body().collect().await.unwrap().to_bytes().to_vec())
}

async fn admin_cookie(app: &Router) -> String {
    let (s, h, body) = send(app, Method::POST, "/api/setup", Some(setup_body("lorenzo", 1)), &[]).await;
    assert_eq!(s, StatusCode::OK, "{body}");
    cookie_of(&h)
}

/// Dati di prova per i backup: gruppi, che l'admin puo' creare.
async fn group_names(app: &Router, cookie: &str) -> Vec<String> {
    let (s, _, meta) = send(app, Method::GET, "/api/meta", None, &[("cookie", cookie)]).await;
    assert_eq!(s, StatusCode::OK, "{meta}");
    meta["groups"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap().to_owned()).collect()
}

async fn add_group(app: &Router, cookie: &str, name: &str) {
    let (s, _, body) = send(app, Method::POST, "/api/groups", Some(json!({"name": name, "kind": "gruppo"})), &[("cookie", cookie)]).await;
    assert_eq!(s, StatusCode::OK, "{body}");
}

async fn backup_now(app: &Router, cookie: &str) -> String {
    let (s, _, item) = send(app, Method::POST, "/api/backups", Some(json!({})), &[("cookie", cookie)]).await;
    assert_eq!(s, StatusCode::OK, "{item}");
    item["name"].as_str().unwrap().to_owned()
}

async fn login_cookie(app: &Router) -> String {
    let (s, h, body) = send(app, Method::POST, "/api/auth/login", Some(json!({"username":"lorenzo","auth_key":key(1)})), &[]).await;
    assert_eq!(s, StatusCode::OK, "{body}");
    cookie_of(&h)
}

#[tokio::test]
async fn backup_and_restore_bring_data_back() {
    let (app, dir) = app(|_| {});
    let cookie = admin_cookie(&app).await;
    add_group(&app, &cookie, "Prima").await;
    std::fs::write(dir.join("themes/mio.json"), b"{\"nome\":\"Mio\"}").unwrap();
    let name = backup_now(&app, &cookie).await;
    add_group(&app, &cookie, "Dopo").await;
    std::fs::remove_file(dir.join("themes/mio.json")).unwrap();

    // Senza la password giusta non si ripristina nulla.
    let uri = format!("/api/backups/{name}/restore");
    let (s, _, _) = send(&app, Method::POST, &uri, Some(json!({"auth_key":key(9)})), &[("cookie", &cookie)]).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    assert!(group_names(&app, &cookie).await.contains(&"Dopo".to_owned()));

    let (s, _, body) = send(&app, Method::POST, &uri, Some(json!({"auth_key":key(1)})), &[("cookie", &cookie)]).await;
    assert_eq!(s, StatusCode::OK, "{body}");
    let (s, _, _) = send(&app, Method::GET, "/api/me", None, &[("cookie", &cookie)]).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED, "dopo il ripristino si accede di nuovo");

    let cookie = login_cookie(&app).await;
    let tags = group_names(&app, &cookie).await;
    assert!(tags.contains(&"Prima".to_owned()) && !tags.contains(&"Dopo".to_owned()), "{tags:?}");
    assert!(dir.join("themes/mio.json").exists(), "tornano anche i temi");
    // La copia di sicurezza contiene lo stato di prima del ripristino, e il database resta scrivibile.
    let (_, _, list) = send(&app, Method::GET, "/api/backups", None, &[("cookie", &cookie)]).await;
    assert!(list["items"].as_array().unwrap().iter().any(|i| i["name"] == body["safety"] && i["kind"] == "sicurezza"), "{list}");
    add_group(&app, &cookie, "Ancora").await;
    let leftovers = std::fs::read_dir(&dir).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().starts_with(".lavoro-")).count();
    assert_eq!(leftovers, 0, "nessun file di lavoro con le chiavi lasciato in giro");
}

#[tokio::test]
async fn a_downloaded_backup_restores_a_fresh_install() {
    let (old, _d1) = app(|_| {});
    let cookie = admin_cookie(&old).await;
    add_group(&old, &cookie, "Salvata").await;
    let name = backup_now(&old, &cookie).await;
    let (s, bytes) = download(&old, &name, &cookie).await;
    assert_eq!(s, StatusCode::OK);
    // Con SQLCipher anche la copia dentro l'archivio deve essere cifrata (senza, e' SQLite in chiaro).
    let plain = bytes.windows(15).any(|w| w == b"SQLite format 3");
    assert_eq!(plain, !cfg!(feature = "sqlcipher"));

    // Installazione nuova, con chiavi diverse. Dal tunnel non si puo'.
    let (new, dir) = app(|_| {});
    let (s, _) = send_bytes(&new, "/api/setup/restore", bytes.clone(), &[("cf-connecting-ip", "203.0.113.9")]).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = send_bytes(&new, "/api/setup/restore", b"non un backup".to_vec(), &[]).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, body) = send_bytes(&new, "/api/setup/restore", bytes.clone(), &[]).await;
    assert_eq!(s, StatusCode::OK, "{body}");
    let cookie = login_cookie(&new).await;
    assert!(group_names(&new, &cookie).await.contains(&"Salvata".to_owned()));
    let (s, _) = send_bytes(&new, "/api/setup/restore", bytes.clone(), &[]).await;
    assert_eq!(s, StatusCode::CONFLICT, "solo su un'installazione vuota");

    // Dopo un riavvio il database ripristinato si apre con la chiave di questa installazione.
    drop(new);
    let mut cfg = Config::default();
    cfg.paths.data_dir = dir;
    cfg.security.cookie_secure = false;
    let again = router(build_state(cfg).expect("riapertura"));
    let cookie = login_cookie(&again).await;
    assert!(group_names(&again, &cookie).await.contains(&"Salvata".to_owned()));
    let (s, item) = send_bytes(&again, "/api/backups/upload", bytes, &[("cookie", &cookie)]).await;
    assert_eq!((s, item["kind"].clone()), (StatusCode::OK, json!("caricato")), "{item}");
}

#[tokio::test]
async fn backups_are_for_the_admin_only_and_names_cannot_escape() {
    let (app, _dir) = app(|_| {});
    let cookie = admin_cookie(&app).await;
    let name = backup_now(&app, &cookie).await;
    for (method, uri) in [(Method::GET, "/api/backups".to_owned()), (Method::GET, format!("/api/backups/{name}")), (Method::POST, "/api/backups".to_owned())] {
        let (s, _, _) = send(&app, method, &uri, None, &[]).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED, "{uri}");
    }
    let (s, _) = send_bytes(&app, "/api/backups/upload", vec![0; 10], &[]).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let (s, _) = download(&app, "..%2Fsecrets%2Fdb.key", &cookie).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _, _) = send(&app, Method::POST, &format!("/api/backups/{name}/delete"), Some(json!({})), &[("cookie", &cookie)]).await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = download(&app, &name, &cookie).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn automatic_backups_run_once_per_slot_keep_the_newest_and_are_copied_outside() {
    let outside = std::env::temp_dir().join(format!("taskhit-copie-{}", auth::random_hex(8)));
    std::fs::create_dir_all(&outside).unwrap();
    let dir = std::env::temp_dir().join(format!("taskhit-test-{}", auth::random_hex(8)));
    let mut cfg = Config::default();
    cfg.paths.data_dir = dir.clone();
    cfg.paths.backup_copy_dir = Some(outside.clone());
    cfg.security.cookie_secure = false;
    let state = build_state(cfg).unwrap();
    let app = router(state.clone());

    assert!(backup::run_due(&state).await.unwrap().is_none(), "senza utenti non c'e' nulla da salvare");
    let cookie = admin_cookie(&app).await;
    let first = backup::run_due(&state).await.unwrap().expect("il primo parte subito");
    assert!(backup::run_due(&state).await.unwrap().is_none(), "uno solo per orario programmato");
    assert!(outside.join(&first.name).is_file());

    let bad = json!({"enabled": true, "days": [], "time": "03:00", "keep": 2});
    let (s, _, _) = send(&app, Method::PUT, "/api/backups/settings", Some(bad), &[("cookie", &cookie)]).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let good = json!({"enabled": false, "days": [1, 5], "time": "18:30", "keep": 2});
    let (s, _, body) = send(&app, Method::PUT, "/api/backups/settings", Some(good), &[("cookie", &cookie)]).await;
    assert_eq!(s, StatusCode::OK, "{body}");
    for _ in 0..3 {
        backup_now(&app, &cookie).await;
    }
    let (_, _, list) = send(&app, Method::GET, "/api/backups", None, &[("cookie", &cookie)]).await;
    assert_eq!(list["items"].as_array().unwrap().len(), 2, "{list}");
    assert_eq!(list["next"], Value::Null, "spenti: nessun prossimo orario");
    assert_eq!(list["copy"]["latest_copied"], json!(true));
}
