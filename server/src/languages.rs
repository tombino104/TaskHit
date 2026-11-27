//! Lingue: un file JSON per lingua, come i temi. L'italiano sta nell'interfaccia; qui ci sono le traduzioni.
//! Quelle incluse viaggiano col programma (cosi' si aggiornano con l'app); un file nella cartella
//! `languages` aggiunge una lingua, oppure ritocca i testi di una inclusa.

use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::Path};

pub const MAX_FILE_BYTES: u64 = 256 * 1024;
const MAX_ENTRIES: usize = 4000;
const MAX_TEXT_CHARS: usize = 1000;

const BUILT_IN: [(&str, &str); 1] = [("en", include_str!("../languages-default/en.json"))];

#[derive(Debug, Deserialize)]
struct LanguageFile {
    nome: String,
    testi: BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
pub struct Language {
    pub id: String,
    pub nome: String,
    pub testi: BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
pub struct Summary {
    pub id: String,
    pub nome: String,
}

#[derive(Debug, Serialize)]
pub struct InvalidLanguage {
    pub file: String,
    pub error: String,
}

#[derive(Debug, Serialize, Default)]
pub struct LanguageList {
    pub languages: Vec<Summary>,
    pub invalid: Vec<InvalidLanguage>,
}

/// "en", "fr", "pt-br".
fn valid_id(id: &str) -> bool {
    (2..=10).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_lowercase() || b == b'-')
}

fn parse(text: &str) -> Result<LanguageFile, String> {
    let file: LanguageFile = serde_json::from_str(text).map_err(|e| format!("JSON non valido: {e}"))?;
    if file.nome.trim().is_empty() || file.nome.chars().count() > 40 {
        return Err("'nome' deve avere da 1 a 40 caratteri".into());
    }
    if file.testi.len() > MAX_ENTRIES || file.testi.iter().any(|(k, v)| k.chars().count() > MAX_TEXT_CHARS || v.chars().count() > MAX_TEXT_CHARS) {
        return Err("troppi testi, o testi troppo lunghi".into());
    }
    Ok(file)
}

fn read(path: &Path) -> Result<LanguageFile, String> {
    let size = fs::metadata(path).map_err(|e| format!("non leggibile: {e}"))?.len();
    if size > MAX_FILE_BYTES {
        return Err("file troppo grande".into());
    }
    parse(&fs::read_to_string(path).map_err(|e| e.to_string())?)
}

/// Tutte le lingue valide, per identificativo: prima quelle incluse, poi i file della cartella sopra di esse.
fn load_all(dir: &Path) -> (BTreeMap<String, LanguageFile>, Vec<InvalidLanguage>) {
    let mut all: BTreeMap<String, LanguageFile> = BUILT_IN.iter().filter_map(|(id, text)| parse(text).ok().map(|f| ((*id).to_owned(), f))).collect();
    let mut invalid = vec![];
    let Ok(entries) = fs::read_dir(dir) else { return (all, invalid) };
    let mut files: Vec<_> = entries.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json")).collect();
    files.sort();
    for path in files {
        let file = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let id = path.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let parsed = if valid_id(&id) { read(&path) } else { Err("il nome del file deve essere il codice della lingua, per esempio fr.json".into()) };
        match parsed {
            Ok(new) => match all.get_mut(&id) {
                Some(base) => {
                    base.nome = new.nome;
                    base.testi.extend(new.testi);
                }
                None => {
                    all.insert(id, new);
                }
            },
            Err(error) => invalid.push(InvalidLanguage { file, error }),
        }
    }
    (all, invalid)
}

pub fn list(dir: &Path) -> LanguageList {
    let (all, invalid) = load_all(dir);
    LanguageList { languages: all.into_iter().map(|(id, f)| Summary { id, nome: f.nome }).collect(), invalid }
}

pub fn get(dir: &Path, id: &str) -> Option<Language> {
    let (mut all, _) = load_all(dir);
    all.remove(id).map(|f| Language { id: id.to_owned(), nome: f.nome, testi: f.testi })
}
