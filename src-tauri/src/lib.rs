use rusqlite::Connection;
use std::sync::Mutex;
use serde::Serialize; // Necessário para enviar dados para o React

pub struct DbState(pub Mutex<Connection>);

// Estrutura que o React vai receber
#[derive(Serialize)]
pub struct Note {
    id: i32,
    title: String,
    content: String,
}

fn init_db() -> Connection {
    let conn = Connection::open("study_platform.db").expect("Falha ao abrir o banco de dados");
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
        END;"
    ).expect("Falha ao inicializar o schema");
    conn
}

#[tauri::command]
fn save_note(state: tauri::State<DbState>, title: String, content: String) -> Result<(), String> {
    let conn = state.inner().0.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO notes (title, content) VALUES (?1, ?2)",
        rusqlite::params![title, content],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

// NOVO: Buscar todas as notas (para carregar ao abrir o app)
#[tauri::command]
fn get_all_notes(state: tauri::State<DbState>) -> Result<Vec<Note>, String> {
    let conn = state.inner().0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare("SELECT id, title, content FROM notes ORDER BY id DESC").map_err(|e| e.to_string())?;
    
    let notes = stmt.query_map([], |row| {
        Ok(Note {
            id: row.get(0)?,
            title: row.get(1)?,
            content: row.get(2)?,
        })
    }).map_err(|e| e.to_string())?
    .collect::<Result<Vec<Note>, _>>()
    .map_err(|e| e.to_string())?;
    
    Ok(notes)
}

// ATUALIZADO: Agora retorna Vec<Note> em vez de Vec<String>
#[tauri::command]
fn search_notes(state: tauri::State<DbState>, query: String) -> Result<Vec<Note>, String> {
    let conn = state.inner().0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT notes.id, notes.title, notes.content 
         FROM notes_fts 
         JOIN notes ON notes_fts.rowid = notes.id 
         WHERE notes_fts MATCH ?1 ORDER BY rank"
    ).map_err(|e| e.to_string())?;
    
    let notes = stmt.query_map(rusqlite::params![query], |row| {
        Ok(Note {
            id: row.get(0)?,
            title: row.get(1)?,
            content: row.get(2)?,
        })
    }).map_err(|e| e.to_string())?
    .collect::<Result<Vec<Note>, _>>()
    .map_err(|e| e.to_string())?;
    
    Ok(notes)
}

// NOVO: Comando para deletar nota
#[tauri::command]
fn delete_note(state: tauri::State<DbState>, id: i32) -> Result<(), String> {
    let conn = state.inner().0.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM notes WHERE id = ?1", rusqlite::params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let conn = init_db();
    
    tauri::Builder::default()
        .manage(DbState(Mutex::new(conn)))
        // Registre todos os comandos aqui:
        .invoke_handler(tauri::generate_handler![save_note, get_all_notes, search_notes, delete_note])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}