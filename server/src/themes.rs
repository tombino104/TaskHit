//! Temi: un solo file JSON per tema, letto da tutta l'app.
//! Il server li valida (colori, contrasto, nomi dei font) prima di offrirli ai client,
//! cosi' un tema illeggibile o malformato non puo' rompere l'interfaccia.

use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::Path};

pub const COLOR_KEYS: [&str; 8] = ["sfondo", "superficie", "inchiostro", "inchiostro2", "attenuato", "filetto", "accento", "suAccento"];
const FONT_KEYS: [&str; 3] = ["titoli", "testo", "dati"];
const MAX_FILE_BYTES: u64 = 16 * 1024;

const DEFAULTS: [(&str, &str); 4] = [
    ("carta", include_str!("../themes-default/carta.json")),
    ("notte", include_str!("../themes-default/notte.json")),
    ("quaderno", include_str!("../themes-default/quaderno.json")),
    ("lavagna", include_str!("../themes-default/lavagna.json")),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    #[serde(default, skip_deserializing)]
    pub id: String,
    pub nome: String,
    pub tipo: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abbinato: Option<String>,
    pub colori: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub font: BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
pub struct InvalidTheme {
    pub file: String,
    pub error: String,
}

#[derive(Debug, Serialize, Default)]
pub struct ThemeList {
    pub themes: Vec<Theme>,
    pub invalid: Vec<InvalidTheme>,
}

fn parse_hex(c: &str) -> Option<[f64; 3]> {
    let h = c.strip_prefix('#')?;
    if h.len() != 6 || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let ch = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok().map(f64::from);
    Some([ch(0)?, ch(2)?, ch(4)?])
}

fn luminance(rgb: [f64; 3]) -> f64 {
    let lin = |v: f64| {
        let s = v / 255.0;
        if s <= 0.03928 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * lin(rgb[0]) + 0.7152 * lin(rgb[1]) + 0.0722 * lin(rgb[2])
}

/// Rapporto di contrasto WCAG tra due colori esadecimali.
pub fn contrast(a: &str, b: &str) -> Option<f64> {
    let (la, lb) = (luminance(parse_hex(a)?), luminance(parse_hex(b)?));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    Some((hi + 0.05) / (lo + 0.05))
}

pub fn validate(t: &Theme) -> Result<(), String> {
    if t.nome.trim().is_empty() || t.nome.chars().count() > 40 {
        return Err("'nome' deve avere da 1 a 40 caratteri".into());
    }
    if t.tipo != "chiaro" && t.tipo != "scuro" {
        return Err("'tipo' deve essere \"chiaro\" o \"scuro\"".into());
    }
    for k in COLOR_KEYS {
        match t.colori.get(k) {
            None => return Err(format!("manca il colore '{k}'")),
            Some(v) if parse_hex(v).is_none() => return Err(format!("il colore '{k}' deve essere #RRGGBB")),
            _ => {}
        }
    }
    if let Some(extra) = t.colori.keys().find(|k| !COLOR_KEYS.contains(&k.as_str())) {
        return Err(format!("colore sconosciuto '{extra}'"));
    }
    for (k, v) in &t.font {
        if !FONT_KEYS.contains(&k.as_str()) {
            return Err(format!("font sconosciuto '{k}'"));
        }
        // Solo lettere, numeri e spazi: il nome finisce in una variabile CSS.
        if v.is_empty() || v.len() > 40 || !v.chars().all(|c| c.is_ascii_alphanumeric() || c == ' ') {
            return Err(format!("il font '{k}' ha un nome non valido"));
        }
    }
    let c = |k: &str| t.colori[k].as_str();
    let checks = [
        ("inchiostro", "sfondo", 7.0),
        ("inchiostro", "superficie", 7.0),
        ("inchiostro2", "sfondo", 4.5),
        ("attenuato", "sfondo", 4.5),
        ("attenuato", "superficie", 4.5),
        ("accento", "sfondo", 4.5),
        ("suAccento", "accento", 4.5),
    ];
    for (fg, bg, min) in checks {
        let ratio = contrast(c(fg), c(bg)).unwrap_or(0.0);
        if ratio < min {
            return Err(format!("contrasto troppo basso tra '{fg}' e '{bg}' ({ratio:.1}:1, minimo {min}:1)"));
        }
    }
    Ok(())
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 40 && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Copia i temi predefiniti solo se la cartella non contiene ancora nessun .json.
pub fn seed_defaults(dir: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    let has_any = fs::read_dir(dir)?.filter_map(Result::ok).any(|e| e.path().extension().is_some_and(|x| x == "json"));
    if !has_any {
        for (id, body) in DEFAULTS {
            fs::write(dir.join(format!("{id}.json")), body)?;
        }
    }
    Ok(())
}

pub fn load_dir(dir: &Path) -> ThemeList {
    let mut out = ThemeList::default();
    let Ok(rd) = fs::read_dir(dir) else { return out };
    let mut files: Vec<_> = rd.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json")).collect();
    files.sort();
    for path in files {
        let file = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let id = path.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let mut bad = |error: String| out.invalid.push(InvalidTheme { file: file.clone(), error });
        if !valid_id(&id) {
            bad("il nome del file deve usare solo minuscole, numeri e trattini".into());
            continue;
        }
        match fs::metadata(&path) {
            Ok(m) if m.len() > MAX_FILE_BYTES => { bad("file troppo grande".into()); continue; }
            Err(e) => { bad(format!("non leggibile: {e}")); continue; }
            _ => {}
        }
        let parsed = fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|s| serde_json::from_str::<Theme>(&s).map_err(|e| format!("JSON non valido: {e}")));
        match parsed {
            Ok(mut t) => match validate(&t) {
                Ok(()) => { t.id = id; out.themes.push(t); }
                Err(e) => bad(e),
            },
            Err(e) => bad(e),
        }
    }
    out
}
