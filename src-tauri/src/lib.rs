use tauri::Manager;
use rusqlite::{params, params_from_iter, Connection};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;
mod planner;

pub struct DbState(pub Mutex<Connection>);

// Estrutura que o React vai receber
#[derive(Serialize)]
pub struct Note {
    id: i32,
    title: String,
    content: String,
    tags: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    id: i64,
    name: String,
    note_count: i64,
}

fn init_db(path: &std::path::Path) -> Connection {
    let conn = Connection::open(path).expect("Falha ao abrir o banco de dados");

        conn.execute_batch("PRAGMA foreign_keys = ON;")
        .expect("Falha ao ativar foreign_keys");

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS notes (
            id INTEGER PRIMARY KEY,
            title TEXT NOT NULL,
            content TEXT NOT NULL
        );
        CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5(
            title, content, content='notes', content_rowid='id'
        );
        CREATE TRIGGER IF NOT EXISTS notes_ai AFTER INSERT ON notes BEGIN
            INSERT INTO notes_fts(rowid, title, content) VALUES (new.id, new.title, new.content);
        END;
        CREATE TRIGGER IF NOT EXISTS notes_ad AFTER DELETE ON notes BEGIN
            INSERT INTO notes_fts(notes_fts, rowid, title, content) VALUES('delete', old.id, old.title, old.content);
        END;
        CREATE TRIGGER IF NOT EXISTS notes_au AFTER UPDATE ON notes BEGIN
            INSERT INTO notes_fts(notes_fts, rowid, title, content) VALUES('delete', old.id, old.title, old.content);
            INSERT INTO notes_fts(rowid, title, content) VALUES (new.id, new.title, new.content);
        END;

        CREATE TABLE IF NOT EXISTS tags (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE COLLATE NOCASE
        );
        CREATE TABLE IF NOT EXISTS note_tags (
            note_id INTEGER NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
            tag_id  INTEGER NOT NULL REFERENCES tags(id)  ON DELETE CASCADE,
            PRIMARY KEY (note_id, tag_id)
        );
        CREATE INDEX IF NOT EXISTS idx_note_tags_tag ON note_tags(tag_id);"
    ).expect("Falha ao inicializar o schema");

    planner::migrate(&conn).expect("Falha ao migrar o planejador");

    conn
}

/// "  #Rust   Lang " -> "rust lang" (máx. 40 caracteres)
fn normalize_tag(raw: &str) -> String {
    raw.trim()
        .trim_start_matches('#')
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
        .chars()
        .take(40)
        .collect()
}

/// Substitui todas as tags de uma nota. Chame dentro de uma transação.
fn replace_note_tags(conn: &Connection, note_id: i32, tags: &[String]) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM note_tags WHERE note_id = ?1", params![note_id])?;

    for raw in tags {
        let name = normalize_tag(raw);
        if name.is_empty() {
            continue;
        }
        conn.execute("INSERT OR IGNORE INTO tags (name) VALUES (?1)", params![name])?;
        let tag_id: i64 =
            conn.query_row("SELECT id FROM tags WHERE name = ?1", params![name], |r| r.get(0))?;
        conn.execute(
            "INSERT OR IGNORE INTO note_tags (note_id, tag_id) VALUES (?1, ?2)",
            params![note_id, tag_id],
        )?;
    }

    cleanup_orphan_tags(conn)
}

fn cleanup_orphan_tags(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM tags WHERE id NOT IN (SELECT DISTINCT tag_id FROM note_tags)",
        [],
    )?;
    Ok(())
}

/// note_id -> lista de tags (uma única query para todas as notas)
fn load_tags_map(conn: &Connection) -> rusqlite::Result<HashMap<i32, Vec<String>>> {
    let mut stmt = conn.prepare(
        "SELECT nt.note_id, t.name
         FROM note_tags nt
         JOIN tags t ON t.id = nt.tag_id
         ORDER BY t.name",
    )?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, i32>(0)?, r.get::<_, String>(1)?)))?;

    let mut map: HashMap<i32, Vec<String>> = HashMap::new();
    for row in rows {
        let (id, name) = row?;
        map.entry(id).or_default().push(name);
    }
    Ok(map)
}

fn note_tags(conn: &Connection, note_id: i32) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT t.name FROM tags t
         JOIN note_tags nt ON nt.tag_id = t.id
         WHERE nt.note_id = ?1
         ORDER BY t.name",
    )?;
    let rows = stmt.query_map(params![note_id], |r| r.get(0))?;
    rows.collect()
}

#[tauri::command]
fn save_note(
    state: tauri::State<DbState>,
    title: String,
    content: String,
    tags: Vec<String>,
) -> Result<(), String> {
    let mut conn = state.inner().0.lock().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    tx.execute(
        "INSERT INTO notes (title, content) VALUES (?1, ?2)",
        params![title, content],
    ).map_err(|e| e.to_string())?;

    let note_id = tx.last_insert_rowid() as i32;
    replace_note_tags(&tx, note_id, &tags).map_err(|e| e.to_string())?;

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

// Substitui get_all_notes + search_notes: texto e tag são ambos opcionais.
#[tauri::command]
fn search_notes(
    state: tauri::State<DbState>,
    query: String,
    tag: Option<String>,
) -> Result<Vec<Note>, String> {
    let conn = state.inner().0.lock().map_err(|e| e.to_string())?;

    let q = query.trim().to_string();
    let tag = tag.map(|t| normalize_tag(&t)).filter(|t| !t.is_empty());

    let mut sql = String::from("SELECT n.id, n.title, n.content FROM notes n");
    let mut sql_params: Vec<String> = Vec::new();
    let mut conds: Vec<String> = Vec::new();

    if !q.is_empty() {
        sql.push_str(" JOIN notes_fts ON notes_fts.rowid = n.id");
        sql_params.push(q.clone());
        conds.push(format!("notes_fts MATCH ?{}", sql_params.len()));
    }
    if let Some(t) = tag {
        sql_params.push(t);
        conds.push(format!(
            "n.id IN (SELECT nt.note_id FROM note_tags nt
                      JOIN tags t ON t.id = nt.tag_id
                      WHERE t.name = ?{})",
            sql_params.len()
        ));
    }
    if !conds.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&conds.join(" AND "));
    }
    sql.push_str(if q.is_empty() { " ORDER BY n.id DESC" } else { " ORDER BY rank" });

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let mut notes = stmt
        .query_map(params_from_iter(sql_params.iter()), |row| {
            Ok(Note {
                id: row.get(0)?,
                title: row.get(1)?,
                content: row.get(2)?,
                tags: Vec::new(),
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<Note>, _>>()
        .map_err(|e| e.to_string())?;

    let mut map = load_tags_map(&conn).map_err(|e| e.to_string())?;
    for n in notes.iter_mut() {
        n.tags = map.remove(&n.id).unwrap_or_default();
    }

    Ok(notes)
}

#[tauri::command]
fn list_tags(state: tauri::State<DbState>) -> Result<Vec<Tag>, String> {
    let conn = state.inner().0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT t.id, t.name, COUNT(nt.note_id)
             FROM tags t
             LEFT JOIN note_tags nt ON nt.tag_id = t.id
             GROUP BY t.id
             ORDER BY t.name",
        )
        .map_err(|e| e.to_string())?;

    let tags = stmt
        .query_map([], |r| {
            Ok(Tag { id: r.get(0)?, name: r.get(1)?, note_count: r.get(2)? })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<Tag>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(tags)
}

#[tauri::command]
fn set_note_tags(
    state: tauri::State<DbState>,
    note_id: i32,
    tags: Vec<String>,
) -> Result<Vec<String>, String> {
    let mut conn = state.inner().0.lock().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    replace_note_tags(&tx, note_id, &tags).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;

    note_tags(&conn, note_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_note(state: tauri::State<DbState>, id: i32) -> Result<(), String> {
    let conn = state.inner().0.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM notes WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    // note_tags some via CASCADE; aqui removemos tags que ficaram sem nota.
    cleanup_orphan_tags(&conn).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let conn = init_db(&dir.join("study_platform.db"));
            app.manage(DbState(Mutex::new(conn)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
    save_note,
    search_notes,
    delete_note,
    list_tags,
    set_note_tags,
    planner::get_week,
    planner::get_overdue,
    planner::add_item,
    planner::toggle_item,
    planner::rename_item,
    planner::set_item_qty,
    planner::set_item_note,
    planner::delete_item,
    planner::move_item,
    planner::list_template,
    planner::add_template_item,
    planner::update_template_item,
    planner::delete_template_item,
    planner::sync_template,
    planner::list_note_titles,
    planner::get_note
])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}