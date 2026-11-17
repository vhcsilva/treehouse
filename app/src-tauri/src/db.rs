//! Armazenamento local (SQLite em %APPDATA%\Treehouse). Documentos JSON por coleção:
//! o formato de cada documento é definido no frontend (`app/src/lib/types.ts`).

use std::path::Path;

use rusqlite::{params, Connection};
#[cfg(test)]
use rusqlite::OptionalExtension;
use serde_json::Value;

pub const COLLECTIONS: &[&str] = &["workspaces", "features", "settings"];

const MIGRATIONS: &[&str] = &[
    // v1
    "CREATE TABLE docs (
        collection TEXT NOT NULL,
        id         TEXT NOT NULL,
        json       TEXT NOT NULL,
        updated_at INTEGER NOT NULL DEFAULT (unixepoch()),
        PRIMARY KEY (collection, id)
    );",
];

pub struct Db {
    conn: Connection,
}

impl Db {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        Self::init(Connection::open(path)?)
    }

    #[cfg(test)]
    pub fn in_memory() -> anyhow::Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> anyhow::Result<Self> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(version as usize) {
            conn.execute_batch(sql)?;
            conn.pragma_update(None, "user_version", (i + 1) as i64)?;
        }
        Ok(Self { conn })
    }

    fn check(collection: &str) -> anyhow::Result<()> {
        anyhow::ensure!(COLLECTIONS.contains(&collection), "coleção desconhecida: {collection}");
        Ok(())
    }

    pub fn list(&self, collection: &str) -> anyhow::Result<Vec<Value>> {
        Self::check(collection)?;
        let mut stmt = self.conn.prepare("SELECT json FROM docs WHERE collection = ?1 ORDER BY rowid")?;
        let rows = stmt.query_map([collection], |r| r.get::<_, String>(0))?;
        rows.map(|r| Ok(serde_json::from_str(&r?)?)).collect()
    }

    #[cfg(test)]
    pub fn get(&self, collection: &str, id: &str) -> anyhow::Result<Option<Value>> {
        Self::check(collection)?;
        let s: Option<String> = self
            .conn
            .query_row("SELECT json FROM docs WHERE collection = ?1 AND id = ?2", [collection, id], |r| r.get(0))
            .optional()?;
        Ok(s.map(|s| serde_json::from_str(&s)).transpose()?)
    }

    pub fn put(&self, collection: &str, id: &str, value: &Value) -> anyhow::Result<()> {
        Self::check(collection)?;
        self.conn.execute(
            "INSERT INTO docs (collection, id, json) VALUES (?1, ?2, ?3)
             ON CONFLICT(collection, id) DO UPDATE SET json = excluded.json, updated_at = unixepoch()",
            params![collection, id, value.to_string()],
        )?;
        Ok(())
    }

    pub fn delete(&self, collection: &str, id: &str) -> anyhow::Result<()> {
        Self::check(collection)?;
        self.conn.execute("DELETE FROM docs WHERE collection = ?1 AND id = ?2", [collection, id])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn crud_and_order() {
        let db = Db::in_memory().unwrap();
        db.put("features", "b", &json!({"id":"b","n":1})).unwrap();
        db.put("features", "a", &json!({"id":"a"})).unwrap();
        db.put("features", "b", &json!({"id":"b","n":2})).unwrap();
        let all = db.list("features").unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0]["n"], 2, "atualizar mantém a ordem de criação");
        assert_eq!(db.get("features", "a").unwrap().unwrap()["id"], "a");
        db.delete("features", "a").unwrap();
        assert!(db.get("features", "a").unwrap().is_none());
        assert!(db.list("nada").is_err());
    }
}
