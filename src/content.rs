//! Couche de contenu d'ArchCore (`content/` du dépôt) : fichiers `<AAAAMMJJHHMM>_<sujet>.sql`, rejoués
//! par `archcore import` et `archcore content` sur la base monde, chacun dans une transaction.
//!
//! Les règles sont celles de `content/README.md` : données seulement (pas de `CREATE`, `ALTER`, `DROP`…,
//! qui valideraient la transaction en cours), requêtes rejouables, aucune donnée copiée du client.

use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize)]
pub struct ContentFile {
    pub name: String,
    pub size: u64,
}

/// Nom d'un fichier de contenu : 12 chiffres, un souligné, un sujet en minuscules.
pub fn valid_name(name: &str) -> bool {
    let Some(stem) = name.strip_suffix(".sql") else { return false };
    let (stamp, subject) = match stem.split_once('_') {
        Some(parts) => parts,
        None => return false,
    };
    stamp.len() == 12
        && stamp.bytes().all(|b| b.is_ascii_digit())
        && !subject.is_empty()
        && subject.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

/// Sujet tiré d'un libellé libre : « Butin du Jeune Hydre » → « butin_du_jeune_hydre ».
pub fn slug(label: &str) -> String {
    let mut out = String::new();
    for c in label.chars() {
        let c = match c {
            'à' | 'â' | 'ä' | 'À' | 'Â' | 'Ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'e',
            'î' | 'ï' | 'Î' | 'Ï' => 'i',
            'ô' | 'ö' | 'Ô' | 'Ö' => 'o',
            'ù' | 'û' | 'ü' | 'Ù' | 'Û' | 'Ü' => 'u',
            'ç' | 'Ç' => 'c',
            c => c,
        };
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('_') && !out.is_empty() {
            out.push('_');
        }
    }
    let out = out.trim_end_matches('_');
    let out: String = out.chars().take(48).collect();
    if out.is_empty() { "contenu".into() } else { out }
}

/// Texte sans les chaînes ni les commentaires, pour y chercher les mots-clés interdits.
fn strip_literals(sql: &str) -> String {
    let mut out = String::with_capacity(sql.len());
    let mut chars = sql.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\'' | '"' | '`' => {
                let quote = c;
                while let Some(d) = chars.next() {
                    if d == '\\' {
                        chars.next();
                    } else if d == quote {
                        if chars.peek() == Some(&quote) {
                            chars.next();
                        } else {
                            break;
                        }
                    }
                }
                out.push(' ');
            }
            '-' if chars.peek() == Some(&'-') => {
                for d in chars.by_ref() {
                    if d == '\n' {
                        break;
                    }
                }
                out.push('\n');
            }
            '#' => {
                for d in chars.by_ref() {
                    if d == '\n' {
                        break;
                    }
                }
                out.push('\n');
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut prev = ' ';
                for d in chars.by_ref() {
                    if prev == '*' && d == '/' {
                        break;
                    }
                    prev = d;
                }
                out.push(' ');
            }
            c => out.push(c),
        }
    }
    out
}

/// Instructions refusées dans la couche de contenu.
pub fn forbidden_statements(sql: &str) -> Vec<String> {
    const FORBIDDEN: [&str; 9] = ["CREATE", "ALTER", "DROP", "TRUNCATE", "RENAME", "GRANT", "REVOKE", "START", "COMMIT"];
    let text = strip_literals(sql).to_ascii_uppercase();
    let mut found = Vec::new();
    for word in text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')) {
        if FORBIDDEN.contains(&word) && !found.iter().any(|f| f == word) {
            found.push(word.to_string());
        }
    }
    found
}

pub struct ContentDir {
    pub path: PathBuf,
}

impl ContentDir {
    pub fn list(&self) -> Result<Vec<ContentFile>> {
        let mut files = Vec::new();
        if !self.path.is_dir() {
            return Ok(files);
        }
        for entry in fs::read_dir(&self.path)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if valid_name(&name) {
                files.push(ContentFile { name, size: entry.metadata()?.len() });
            }
        }
        files.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(files)
    }

    fn file(&self, name: &str) -> Result<PathBuf> {
        if !valid_name(name) {
            bail!("nom de fichier de contenu invalide : {name} (attendu : AAAAMMJJHHMM_sujet.sql)");
        }
        Ok(self.path.join(name))
    }

    pub fn read(&self, name: &str) -> Result<String> {
        let path = self.file(name)?;
        fs::read_to_string(&path).with_context(|| format!("lecture de {}", path.display()))
    }

    /// Ajoute un bloc de requêtes à un fichier existant, ou crée le fichier `<maintenant>_<sujet>.sql`.
    /// Renvoie le nom du fichier écrit.
    pub fn append(&self, file: Option<&str>, subject: Option<&str>, title: &str, sql: &str) -> Result<String> {
        let forbidden = forbidden_statements(sql);
        if !forbidden.is_empty() {
            bail!(
                "la couche de contenu n'accepte que des données : {} refusé (le schéma passe par sql/migrations/)",
                forbidden.join(", ")
            );
        }
        let (name, new) = match file {
            Some(name) => (name.to_string(), false),
            None => {
                let stamp = chrono::Local::now().format("%Y%m%d%H%M");
                (format!("{stamp}_{}.sql", slug(subject.unwrap_or(title))), true)
            }
        };
        let path = self.file(&name)?;
        if new {
            fs::create_dir_all(&self.path).with_context(|| format!("création de {}", self.path.display()))?;
            if path.exists() {
                bail!("{name} existe déjà : choisir ce fichier dans la liste pour y ajouter les requêtes");
            }
        } else if !path.exists() {
            bail!("{name} n'existe pas dans {}", self.path.display());
        }
        let mut out = fs::OpenOptions::new().create(true).append(true).open(&path)?;
        let mut block = String::new();
        if new {
            block.push_str("-- Écrit par ArchForge (https://github.com/BlaMacfly/ArchForge). Rejouable : `archcore content`.\n");
        }
        block.push_str(&format!("\n-- {title}\n{}\n", sql.trim_end()));
        out.write_all(block.as_bytes())?;
        Ok(name)
    }
}

/// Dossier `content/` d'un dépôt ArchCore, s'il en a un.
pub fn find_in(archcore: &Path) -> PathBuf {
    archcore.join("content")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noms_de_fichiers() {
        assert!(valid_name("202610221530_butin_hydre.sql"));
        assert!(!valid_name("2026102215_butin.sql"), "horodatage trop court");
        assert!(!valid_name("202610221530_Butin.sql"), "majuscules");
        assert!(!valid_name("202610221530_../x.sql"), "chemin");
        assert!(!valid_name("README.md"));
    }

    #[test]
    fn sujets() {
        assert_eq!(slug("Butin du Jeune Hydre (TID 459)"), "butin_du_jeune_hydre_tid_459");
        assert_eq!(slug("Quête « Premiers pas »"), "quete_premiers_pas");
        assert_eq!(slug("???"), "contenu");
    }

    #[test]
    fn instructions_interdites_hors_chaines() {
        assert!(forbidden_statements("UPDATE item_data SET name = 'Drop table' WHERE tid = 1;").is_empty());
        assert!(forbidden_statements("-- DROP\nDELETE FROM monster_drop WHERE stable_key = 'a#1';").is_empty());
        assert_eq!(forbidden_statements("DROP TABLE item_data; alter table x add y int"), vec!["DROP", "ALTER"]);
        assert_eq!(forbidden_statements("UPDATE t SET a = 'it''s' ; CREATE TABLE z (a int)"), vec!["CREATE"]);
    }

    #[test]
    fn ajout_dans_un_fichier() {
        let dir = std::env::temp_dir().join(format!("archforge-test-{}", std::process::id()));
        let content = ContentDir { path: dir.clone() };
        let name = content.append(None, Some("Essai"), "Essai", "UPDATE item_data SET stack = 5 WHERE tid = 1;").unwrap();
        assert!(name.ends_with("_essai.sql"));
        content.append(Some(&name), None, "Suite", "UPDATE item_data SET stack = 6 WHERE tid = 1;").unwrap();
        let text = content.read(&name).unwrap();
        assert!(text.contains("stack = 5") && text.contains("-- Suite\nUPDATE item_data SET stack = 6"));
        assert!(content.append(None, None, "x", "DROP TABLE item_data").is_err());
        assert_eq!(content.list().unwrap().len(), 1);
        fs::remove_dir_all(dir).unwrap();
    }
}
