use axum::{
    Router,
    body::Body,
    http::{HeaderMap, Method, Request, StatusCode, header},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD as B64};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::path::PathBuf;
use taskhit_server::{auth, build_state, config::Config, router, themes};
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
    assert_eq!(c.get(&format!("{uri}/history"), &c.admin).await.0, StatusCode::NOT_FOUND);
    let (s, _) = c.post(&format!("{uri}/delete"), json!({"rev":1}), &c.admin).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
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
    let (_, made) = c.post("/api/events", body, &c.admin).await;
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
        let (s, b) = c.post("/api/events", json!({"visibility":"public","data":data}), &c.admin).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "campo {field}: {b}");
    }
    let (s, _) = c.post("/api/tags", json!({"name":"Fisica","color":"red"}), &c.admin).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
}
