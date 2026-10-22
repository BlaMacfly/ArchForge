//! Lecture de la configuration d'un dépôt ArchCore : `config/world.conf` donne la connexion à MariaDB et
//! les noms des bases, `content/` la couche de contenu.

use crate::db::Settings;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn is_archcore(dir: &Path) -> bool {
    dir.join("config").join("world.conf").is_file() || dir.join("config").join("world.conf.dist").is_file()
}

/// Dépôt ArchCore désigné par l'option, sinon le dossier courant, sinon `ARCHCORE_DIR`.
pub fn locate(option: Option<PathBuf>) -> Option<PathBuf> {
    let candidates = option
        .into_iter()
        .chain(std::env::current_dir().ok())
        .chain(std::env::var_os("ARCHCORE_DIR").map(PathBuf::from));
    candidates.into_iter().find(|d| is_archcore(d))
}

/// Clés `Nom = valeur` d'un fichier de configuration d'ArchCore (guillemets retirés, commentaires `#`).
pub fn parse_conf(text: &str) -> HashMap<String, String> {
    let mut keys = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            let v = v.trim();
            let v = v.strip_prefix('"').and_then(|v| v.strip_suffix('"')).unwrap_or(v);
            keys.insert(k.trim().to_string(), v.to_string());
        }
    }
    keys
}

/// Connexion par défaut : celle du serveur monde du dépôt, sinon celle de `world.conf.dist`.
pub fn default_settings(dir: Option<&Path>) -> Settings {
    let mut s = Settings {
        host: "127.0.0.1".into(),
        port: 3306,
        user: "root".into(),
        password: String::new(),
        auth: "archcore_auth".into(),
        characters: "archcore_characters".into(),
        world: "archcore_world".into(),
        logs: "archcore_logs".into(),
    };
    let Some(dir) = dir else { return s };
    let conf = ["world.conf", "world.conf.dist"]
        .iter()
        .find_map(|f| std::fs::read_to_string(dir.join("config").join(f)).ok());
    let Some(conf) = conf else { return s };
    let keys = parse_conf(&conf);
    let get = |k: &str| keys.get(k).cloned();
    if let Some(v) = get("Database.Host") { s.host = v; }
    if let Some(v) = get("Database.Port").and_then(|p| p.parse().ok()) { s.port = v; }
    if let Some(v) = get("Database.User") { s.user = v; }
    if let Some(v) = get("Database.Password") { s.password = v; }
    if let Some(v) = get("Database.Auth") { s.auth = v; }
    if let Some(v) = get("Database.Characters") { s.characters = v; }
    if let Some(v) = get("Database.World") { s.world = v; }
    if let Some(v) = get("Database.Logs") { s.logs = v; }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lecture_de_world_conf() {
        let keys = parse_conf("# commentaire\nDatabase.Host = \"10.0.0.2\"\nDatabase.Port = 3307\n  World.Language = fr\n");
        assert_eq!(keys["Database.Host"], "10.0.0.2");
        assert_eq!(keys["Database.Port"], "3307");
        assert_eq!(keys["World.Language"], "fr");
        assert!(!keys.contains_key("# commentaire"));
    }
}
