//! Planejador semanal.
//!
//! - Rotina-modelo (`template_items`) + semanas geradas a partir dela (`plan_items`)
//! - Tarefas com subitens, checkbox, meta numérica e vínculo com notas
//! - Mover/reordenar tarefas entre dias e períodos
//!
//! A lógica fica em funções puras (testáveis com `cargo test`); os comandos
//! Tauri estão no fim do arquivo e só delegam para elas.

use rusqlite::{params, Connection};
use serde::Serialize;

const PERIODS: [&str; 3] = ["morning", "afternoon", "evening"];
const NOW_SQL: &str = "strftime('%Y-%m-%dT%H:%M:%S','now','localtime')";

// ---------------------------------------------------------------- erros

#[derive(Debug)]
pub struct PErr(pub String);

impl From<rusqlite::Error> for PErr {
    fn from(e: rusqlite::Error) -> Self {
        PErr(e.to_string())
    }
}

type R<T> = Result<T, PErr>;

fn bad<T>(msg: &str) -> R<T> {
    Err(PErr(msg.to_string()))
}

// ---------------------------------------------------------------- tipos

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PlanItem {
    pub id: i64,
    pub date: String,
    pub period: String,
    pub title: String,
    pub position: i64,
    pub parent_id: Option<i64>,
    pub done_at: Option<String>,
    pub template_item_id: Option<i64>,
    pub note_id: Option<i64>,
    pub target_qty: Option<i64>,
    pub done_qty: i64,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TemplateItem {
    pub id: i64,
    pub weekday: i64,
    pub period: String,
    pub title: String,
    pub target_qty: Option<i64>,
    pub position: i64,
}

#[derive(Serialize, Debug, Clone)]
pub struct NoteTitle {
    pub id: i64,
    pub title: String,
}

#[derive(Serialize, Debug, Clone)]
pub struct NoteDetail {
    pub id: i64,
    pub title: String,
    pub content: String,
    pub tags: Vec<String>,
}

// ---------------------------------------------------------------- schema

/// Chame depois de criar a tabela `notes` (há FOREIGN KEY para ela).
pub fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS template_items (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            weekday    INTEGER NOT NULL CHECK (weekday BETWEEN 0 AND 6),  -- 0 = segunda
            period     TEXT    NOT NULL CHECK (period IN ('morning','afternoon','evening')),
            title      TEXT    NOT NULL,
            target_qty INTEGER,
            position   INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS plan_items (
            id               INTEGER PRIMARY KEY AUTOINCREMENT,
            date             TEXT    NOT NULL,                             -- 'AAAA-MM-DD' (local)
            period           TEXT    NOT NULL CHECK (period IN ('morning','afternoon','evening')),
            title            TEXT    NOT NULL,
            position         INTEGER NOT NULL DEFAULT 0,
            parent_id        INTEGER REFERENCES plan_items(id) ON DELETE CASCADE,
            done_at          TEXT,                                         -- NULL = pendente
            template_item_id INTEGER REFERENCES template_items(id) ON DELETE SET NULL,
            note_id          INTEGER REFERENCES notes(id) ON DELETE SET NULL,
            target_qty       INTEGER,
            done_qty         INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_plan_items_date   ON plan_items(date);
        CREATE INDEX IF NOT EXISTS idx_plan_items_parent ON plan_items(parent_id);

        -- Semanas que já foram geradas a partir da rotina (evita duplicar).
        CREATE TABLE IF NOT EXISTS generated_weeks (
            week_start TEXT PRIMARY KEY
        );",
    )
}

// ---------------------------------------------------------------- validação

fn check_date(conn: &Connection, s: &str) -> R<()> {
    let norm: Option<String> = conn.query_row("SELECT date(?1)", [s], |r| r.get(0))?;
    if norm.as_deref() == Some(s) {
        Ok(())
    } else {
        bad("Data inválida (use AAAA-MM-DD)")
    }
}

fn check_period(p: &str) -> R<()> {
    if PERIODS.contains(&p) {
        Ok(())
    } else {
        bad("Período inválido")
    }
}

fn clean_title(t: &str) -> R<String> {
    let t: String = t.trim().chars().take(200).collect();
    if t.is_empty() {
        bad("O título não pode ser vazio")
    } else {
        Ok(t)
    }
}

// ---------------------------------------------------------------- leitura

const ITEM_COLS: &str = "id, date, period, title, position, parent_id, done_at, \
                         template_item_id, note_id, target_qty, done_qty";

fn row_to_item(r: &rusqlite::Row) -> rusqlite::Result<PlanItem> {
    Ok(PlanItem {
        id: r.get(0)?,
        date: r.get(1)?,
        period: r.get(2)?,
        title: r.get(3)?,
        position: r.get(4)?,
        parent_id: r.get(5)?,
        done_at: r.get(6)?,
        template_item_id: r.get(7)?,
        note_id: r.get(8)?,
        target_qty: r.get(9)?,
        done_qty: r.get(10)?,
    })
}

/// Insere na semana as tarefas da rotina que ainda não existem nela.
/// Idempotente: usa (template_item_id, date) como chave.
fn sync_week(conn: &Connection, week_start: &str) -> R<usize> {
    let n = conn.execute(
        "INSERT INTO plan_items (date, period, title, position, template_item_id, target_qty)
         SELECT date(?1, '+' || t.weekday || ' days'),
                t.period,
                t.title,
                COALESCE((SELECT MAX(q.position) FROM plan_items q
                          WHERE q.date = date(?1, '+' || t.weekday || ' days')
                            AND q.period = t.period AND q.parent_id IS NULL), -1) + 1 + t.position,
                t.id,
                t.target_qty
         FROM template_items t
         WHERE NOT EXISTS (
             SELECT 1 FROM plan_items p
             WHERE p.template_item_id = t.id
               AND p.date = date(?1, '+' || t.weekday || ' days'))
         ORDER BY t.weekday, t.position, t.id",
        [week_start],
    )?;
    Ok(n)
}

/// Itens da semana que começa em `week_start` (segunda). Com `generate`,
/// gera a semana a partir da rotina na primeira vez (se a rotina não estiver vazia).
pub fn week_items(conn: &mut Connection, week_start: &str, generate: bool) -> R<Vec<PlanItem>> {
    check_date(conn, week_start)?;

    if generate {
        let tx = conn.transaction()?;
        let already: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM generated_weeks WHERE week_start = ?1)",
            [week_start],
            |r| r.get(0),
        )?;
        let has_template: bool =
            tx.query_row("SELECT EXISTS(SELECT 1 FROM template_items)", [], |r| r.get(0))?;
        if !already && has_template {
            sync_week(&tx, week_start)?;
            tx.execute("INSERT INTO generated_weeks (week_start) VALUES (?1)", [week_start])?;
        }
        tx.commit()?;
    }

    let sql = format!(
        "SELECT {ITEM_COLS} FROM plan_items
         WHERE date BETWEEN ?1 AND date(?1, '+6 days')
         ORDER BY date, position, id"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([week_start], row_to_item)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Tarefas (de nível superior) pendentes em datas anteriores a `today`.
pub fn overdue_items(conn: &Connection, today: &str) -> R<Vec<PlanItem>> {
    check_date(conn, today)?;
    let sql = format!(
        "SELECT {ITEM_COLS} FROM plan_items
         WHERE parent_id IS NULL AND done_at IS NULL AND date < ?1
         ORDER BY date, position, id"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([today], row_to_item)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

// ---------------------------------------------------------------- escrita

/// Marca o pai como concluído se (e somente se) todos os subitens estão concluídos.
fn sync_parent(conn: &Connection, parent_id: i64) -> R<()> {
    let (total, pending): (i64, i64) = conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(done_at IS NULL), 0) FROM plan_items WHERE parent_id = ?1",
        [parent_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    if total == 0 {
        return Ok(());
    }
    if pending == 0 {
        conn.execute(
            &format!("UPDATE plan_items SET done_at = COALESCE(done_at, {NOW_SQL}) WHERE id = ?1"),
            [parent_id],
        )?;
    } else {
        conn.execute("UPDATE plan_items SET done_at = NULL WHERE id = ?1", [parent_id])?;
    }
    Ok(())
}

pub fn create_item(
    conn: &mut Connection,
    date: &str,
    period: &str,
    title: &str,
    parent_id: Option<i64>,
) -> R<i64> {
    let title = clean_title(title)?;
    let tx = conn.transaction()?;

    let id = if let Some(pid) = parent_id {
        // Subitem herda data/período do pai (e só há um nível de subitens).
        let (pdate, pperiod): (String, String) = tx
            .query_row(
                "SELECT date, period FROM plan_items WHERE id = ?1 AND parent_id IS NULL",
                [pid],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|_| PErr("Tarefa-pai não encontrada".into()))?;
        let pos: i64 = tx.query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM plan_items WHERE parent_id = ?1",
            [pid],
            |r| r.get(0),
        )?;
        tx.execute(
            "INSERT INTO plan_items (date, period, title, position, parent_id)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![pdate, pperiod, title, pos, pid],
        )?;
        let id = tx.last_insert_rowid();
        sync_parent(&tx, pid)?; // um subitem novo está pendente -> pai volta a pendente
        id
    } else {
        check_date(&tx, date)?;
        check_period(period)?;
        let pos: i64 = tx.query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM plan_items
             WHERE date = ?1 AND period = ?2 AND parent_id IS NULL",
            params![date, period],
            |r| r.get(0),
        )?;
        tx.execute(
            "INSERT INTO plan_items (date, period, title, position) VALUES (?1, ?2, ?3, ?4)",
            params![date, period, title, pos],
        )?;
        tx.last_insert_rowid()
    };

    tx.commit()?;
    Ok(id)
}

/// Concluir/reabrir. Marcar o pai marca todos os subitens; marcar um subitem
/// atualiza o pai (concluído só quando todos os subitens estão concluídos).
pub fn set_done(conn: &mut Connection, id: i64, done: bool) -> R<()> {
    let tx = conn.transaction()?;
    let parent: Option<i64> = tx
        .query_row("SELECT parent_id FROM plan_items WHERE id = ?1", [id], |r| r.get(0))
        .map_err(|_| PErr("Tarefa não encontrada".into()))?;

    if done {
        tx.execute(
            &format!(
                "UPDATE plan_items SET done_at = COALESCE(done_at, {NOW_SQL})
                 WHERE id = ?1 OR parent_id = ?1"
            ),
            [id],
        )?;
    } else {
        tx.execute(
            "UPDATE plan_items SET done_at = NULL WHERE id = ?1 OR parent_id = ?1",
            [id],
        )?;
    }
    if let Some(pid) = parent {
        sync_parent(&tx, pid)?;
    }
    tx.commit()?;
    Ok(())
}

pub fn rename(conn: &Connection, id: i64, title: &str) -> R<()> {
    let title = clean_title(title)?;
    conn.execute("UPDATE plan_items SET title = ?1 WHERE id = ?2", params![title, id])?;
    Ok(())
}

/// `target` None (ou 0) remove a meta.
pub fn set_qty(conn: &Connection, id: i64, target: Option<i64>, done: i64) -> R<()> {
    let target = target.filter(|t| *t > 0).map(|t| t.min(100_000));
    let done = done.clamp(0, 100_000);
    conn.execute(
        "UPDATE plan_items SET target_qty = ?1, done_qty = ?2 WHERE id = ?3",
        params![target, done, id],
    )?;
    Ok(())
}

pub fn link_note(conn: &Connection, id: i64, note_id: Option<i64>) -> R<()> {
    conn.execute("UPDATE plan_items SET note_id = ?1 WHERE id = ?2", params![note_id, id])
        .map_err(|_| PErr("Nota não encontrada".into()))?;
    Ok(())
}

pub fn remove_item(conn: &mut Connection, id: i64) -> R<()> {
    let tx = conn.transaction()?;
    let parent: Option<i64> = tx
        .query_row("SELECT parent_id FROM plan_items WHERE id = ?1", [id], |r| r.get(0))
        .map_err(|_| PErr("Tarefa não encontrada".into()))?;
    tx.execute("DELETE FROM plan_items WHERE id = ?1", [id])?; // subitens saem por CASCADE
    if let Some(pid) = parent {
        sync_parent(&tx, pid)?;
    }
    tx.commit()?;
    Ok(())
}

/// Move uma tarefa (e seus subitens) para outra data/período, antes de `before_id`
/// (ou no fim da célula quando None). Também serve para reordenar.
pub fn relocate_item(
    conn: &mut Connection,
    id: i64,
    date: &str,
    period: &str,
    before_id: Option<i64>,
) -> R<()> {
    check_date(conn, date)?;
    check_period(period)?;
    let tx = conn.transaction()?;

    let parent: Option<i64> = tx
        .query_row("SELECT parent_id FROM plan_items WHERE id = ?1", [id], |r| r.get(0))
        .map_err(|_| PErr("Tarefa não encontrada".into()))?;
    if parent.is_some() {
        return bad("Subitens não podem ser movidos separadamente");
    }

    tx.execute(
        "UPDATE plan_items SET date = ?2, period = ?3 WHERE id = ?1 OR parent_id = ?1",
        params![id, date, period],
    )?;

    let mut ids: Vec<i64> = {
        let mut stmt = tx.prepare(
            "SELECT id FROM plan_items
             WHERE date = ?1 AND period = ?2 AND parent_id IS NULL AND id != ?3
             ORDER BY position, id",
        )?;
        let rows = stmt.query_map(params![date, period, id], |r| r.get(0))?;
        rows.collect::<rusqlite::Result<Vec<i64>>>()?
    };
    let idx = before_id
        .and_then(|b| ids.iter().position(|x| *x == b))
        .unwrap_or(ids.len());
    ids.insert(idx, id);
    for (i, x) in ids.iter().enumerate() {
        tx.execute("UPDATE plan_items SET position = ?1 WHERE id = ?2", params![i as i64, x])?;
    }

    tx.commit()?;
    Ok(())
}

// ---------------------------------------------------------------- rotina-modelo

pub fn template_list(conn: &Connection) -> R<Vec<TemplateItem>> {
    let mut stmt = conn.prepare(
        "SELECT id, weekday, period, title, target_qty, position
         FROM template_items ORDER BY weekday, position, id",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(TemplateItem {
            id: r.get(0)?,
            weekday: r.get(1)?,
            period: r.get(2)?,
            title: r.get(3)?,
            target_qty: r.get(4)?,
            position: r.get(5)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn template_add(
    conn: &Connection,
    weekday: i64,
    period: &str,
    title: &str,
    target_qty: Option<i64>,
) -> R<i64> {
    if !(0..=6).contains(&weekday) {
        return bad("Dia da semana inválido");
    }
    check_period(period)?;
    let title = clean_title(title)?;
    let target = target_qty.filter(|t| *t > 0).map(|t| t.min(100_000));
    let pos: i64 = conn.query_row(
        "SELECT COALESCE(MAX(position), -1) + 1 FROM template_items
         WHERE weekday = ?1 AND period = ?2",
        params![weekday, period],
        |r| r.get(0),
    )?;
    conn.execute(
        "INSERT INTO template_items (weekday, period, title, target_qty, position)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![weekday, period, title, target, pos],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn template_update(conn: &Connection, id: i64, title: &str, target_qty: Option<i64>) -> R<()> {
    let title = clean_title(title)?;
    let target = target_qty.filter(|t| *t > 0).map(|t| t.min(100_000));
    conn.execute(
        "UPDATE template_items SET title = ?1, target_qty = ?2 WHERE id = ?3",
        params![title, target, id],
    )?;
    Ok(())
}

pub fn template_remove(conn: &Connection, id: i64) -> R<()> {
    conn.execute("DELETE FROM template_items WHERE id = ?1", [id])?;
    Ok(())
}

/// "Aplicar rotina à semana": adiciona só o que falta. Retorna quantas tarefas entraram.
pub fn template_sync(conn: &mut Connection, week_start: &str) -> R<usize> {
    check_date(conn, week_start)?;
    let tx = conn.transaction()?;
    let n = sync_week(&tx, week_start)?;
    tx.execute(
        "INSERT OR IGNORE INTO generated_weeks (week_start) VALUES (?1)",
        [week_start],
    )?;
    tx.commit()?;
    Ok(n)
}

// ---------------------------------------------------------------- notas (para o vínculo)

pub fn note_titles(conn: &Connection) -> R<Vec<NoteTitle>> {
    let mut stmt = conn.prepare("SELECT id, title FROM notes ORDER BY title COLLATE NOCASE")?;
    let rows = stmt.query_map([], |r| Ok(NoteTitle { id: r.get(0)?, title: r.get(1)? }))?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn note_detail(conn: &Connection, id: i64) -> R<NoteDetail> {
    let (title, content): (String, String) = conn
        .query_row("SELECT title, content FROM notes WHERE id = ?1", [id], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .map_err(|_| PErr("Nota não encontrada".into()))?;
    let mut stmt = conn.prepare(
        "SELECT t.name FROM tags t JOIN note_tags nt ON nt.tag_id = t.id
         WHERE nt.note_id = ?1 ORDER BY t.name",
    )?;
    let tags = stmt
        .query_map([id], |r| r.get(0))?
        .collect::<rusqlite::Result<Vec<String>>>()?;
    Ok(NoteDetail { id, title, content, tags })
}

// ---------------------------------------------------------------- testes

#[cfg(test)]
mod tests {
    use super::*;

    const MON: &str = "2026-11-23"; // segunda-feira

    fn db() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE notes (id INTEGER PRIMARY KEY, title TEXT NOT NULL, content TEXT NOT NULL);
             CREATE TABLE tags (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL UNIQUE);
             CREATE TABLE note_tags (note_id INTEGER, tag_id INTEGER);",
        )
        .unwrap();
        migrate(&c).unwrap();
        c
    }

    fn tops(items: &[PlanItem], date: &str, period: &str) -> Vec<String> {
        items
            .iter()
            .filter(|i| i.parent_id.is_none() && i.date == date && i.period == period)
            .map(|i| i.title.clone())
            .collect()
    }

    #[test]
    fn migrate_is_repeatable() {
        let c = db();
        migrate(&c).unwrap();
        migrate(&c).unwrap();
    }

    #[test]
    fn rejects_bad_dates_and_periods() {
        let mut c = db();
        assert!(week_items(&mut c, "garbage", false).is_err());
        assert!(week_items(&mut c, "2026-02-31", false).is_err());
        assert!(create_item(&mut c, MON, "night", "x", None).is_err());
        assert!(create_item(&mut c, MON, "morning", "   ", None).is_err());
    }

    #[test]
    fn generation_is_idempotent_and_dated_correctly() {
        let mut c = db();
        template_add(&c, 0, "morning", "Matemática", Some(10)).unwrap();
        template_add(&c, 0, "morning", "Física", None).unwrap();
        template_add(&c, 2, "afternoon", "Bio", None).unwrap();

        let a = week_items(&mut c, MON, true).unwrap();
        assert_eq!(a.len(), 3);
        assert_eq!(tops(&a, "2026-11-23", "morning"), vec!["Matemática", "Física"]);
        assert_eq!(tops(&a, "2026-11-25", "afternoon"), vec!["Bio"]);
        assert_eq!(a.iter().find(|i| i.title == "Matemática").unwrap().target_qty, Some(10));

        // abrir de novo não duplica
        let b = week_items(&mut c, MON, true).unwrap();
        assert_eq!(b.len(), 3);

        // tarefa apagada pelo usuário não ressuscita ao reabrir a semana
        remove_item(&mut c, a[0].id).unwrap();
        assert_eq!(week_items(&mut c, MON, true).unwrap().len(), 2);
    }

    #[test]
    fn no_generation_without_flag_or_without_template() {
        let mut c = db();
        assert!(week_items(&mut c, MON, true).unwrap().is_empty()); // rotina vazia

        template_add(&c, 1, "morning", "Linguagens", None).unwrap();
        assert!(week_items(&mut c, MON, false).unwrap().is_empty()); // passado: não gera
        // rotina vazia não marcou a semana como gerada, então agora gera
        assert_eq!(week_items(&mut c, MON, true).unwrap().len(), 1);
    }

    #[test]
    fn template_sync_adds_only_missing() {
        let mut c = db();
        template_add(&c, 0, "morning", "A", None).unwrap();
        assert_eq!(week_items(&mut c, MON, true).unwrap().len(), 1);

        template_add(&c, 0, "morning", "B", None).unwrap();
        assert_eq!(template_sync(&mut c, MON).unwrap(), 1);
        assert_eq!(template_sync(&mut c, MON).unwrap(), 0);

        let items = week_items(&mut c, MON, false).unwrap();
        assert_eq!(tops(&items, MON, "morning"), vec!["A", "B"]);
    }

    #[test]
    fn subitems_and_parent_sync() {
        let mut c = db();
        let p = create_item(&mut c, MON, "morning", "Bio", None).unwrap();
        let s1 = create_item(&mut c, "ignored", "ignored", "5q. Desacopl.", Some(p)).unwrap();
        let s2 = create_item(&mut c, "ignored", "ignored", "5q. Embrio", Some(p)).unwrap();

        let items = week_items(&mut c, MON, false).unwrap();
        let sub = items.iter().find(|i| i.id == s1).unwrap();
        assert_eq!((sub.date.as_str(), sub.period.as_str()), (MON, "morning")); // herdou do pai

        set_done(&mut c, s1, true).unwrap();
        let parent_done = |c: &mut Connection| {
            week_items(c, MON, false).unwrap().iter().find(|i| i.id == p).unwrap().done_at.is_some()
        };
        assert!(!parent_done(&mut c)); // falta s2

        set_done(&mut c, s2, true).unwrap();
        assert!(parent_done(&mut c)); // todos concluídos -> pai concluído

        let s3 = create_item(&mut c, "", "", "novo", Some(p)).unwrap();
        assert!(!parent_done(&mut c)); // subitem pendente reabre o pai

        remove_item(&mut c, s3).unwrap();
        assert!(parent_done(&mut c)); // sem pendentes de novo

        set_done(&mut c, p, false).unwrap(); // desmarcar o pai desmarca filhos
        let items = week_items(&mut c, MON, false).unwrap();
        assert!(items.iter().all(|i| i.done_at.is_none()));

        set_done(&mut c, p, true).unwrap(); // marcar o pai marca filhos
        let items = week_items(&mut c, MON, false).unwrap();
        assert!(items.iter().all(|i| i.done_at.is_some()));
    }

    #[test]
    fn subitems_cannot_nest_or_move() {
        let mut c = db();
        let p = create_item(&mut c, MON, "morning", "P", None).unwrap();
        let s = create_item(&mut c, "", "", "S", Some(p)).unwrap();
        assert!(create_item(&mut c, "", "", "SS", Some(s)).is_err());
        assert!(relocate_item(&mut c, s, "2026-11-24", "morning", None).is_err());
    }

    #[test]
    fn move_and_reorder_with_children() {
        let mut c = db();
        let a = create_item(&mut c, MON, "morning", "A", None).unwrap();
        let b = create_item(&mut c, MON, "morning", "B", None).unwrap();
        let d = create_item(&mut c, MON, "morning", "C", None).unwrap();
        let child = create_item(&mut c, "", "", "A1", Some(a)).unwrap();

        // reordenar: C antes de A
        relocate_item(&mut c, d, MON, "morning", Some(a)).unwrap();
        let items = week_items(&mut c, MON, false).unwrap();
        assert_eq!(tops(&items, MON, "morning"), vec!["C", "A", "B"]);

        // mover A (com filho) para terça/tarde
        relocate_item(&mut c, a, "2026-11-24", "afternoon", None).unwrap();
        let items = week_items(&mut c, MON, false).unwrap();
        assert_eq!(tops(&items, MON, "morning"), vec!["C", "B"]);
        assert_eq!(tops(&items, "2026-11-24", "afternoon"), vec!["A"]);
        let moved_child = items.iter().find(|i| i.id == child).unwrap();
        assert_eq!(moved_child.date, "2026-11-24");
        assert_eq!(moved_child.period, "afternoon");

        // before_id inexistente nessa célula -> vai para o fim
        relocate_item(&mut c, b, "2026-11-24", "afternoon", Some(99999)).unwrap();
        let items = week_items(&mut c, MON, false).unwrap();
        assert_eq!(tops(&items, "2026-11-24", "afternoon"), vec!["A", "B"]);
    }

    #[test]
    fn overdue_lists_only_past_pending_top_level() {
        let mut c = db();
        let late = create_item(&mut c, "2026-11-23", "morning", "atrasada", None).unwrap();
        let done = create_item(&mut c, "2026-11-23", "morning", "feita", None).unwrap();
        create_item(&mut c, "2026-11-25", "morning", "hoje", None).unwrap();
        create_item(&mut c, "2026-11-26", "morning", "futura", None).unwrap();
        create_item(&mut c, "", "", "filho", Some(late)).unwrap();
        set_done(&mut c, done, true).unwrap();

        let o = overdue_items(&c, "2026-11-25").unwrap();
        assert_eq!(o.len(), 1);
        assert_eq!(o[0].title, "atrasada");

        // "arrastar" para hoje tira da lista de atrasadas
        relocate_item(&mut c, late, "2026-11-25", "morning", None).unwrap();
        assert!(overdue_items(&c, "2026-11-25").unwrap().is_empty());
    }

    #[test]
    fn quantities_are_clamped() {
        let mut c = db();
        let id = create_item(&mut c, MON, "morning", "Q", None).unwrap();
        set_qty(&c, id, Some(5), 3).unwrap();
        let it = week_items(&mut c, MON, false).unwrap().remove(0);
        assert_eq!((it.target_qty, it.done_qty), (Some(5), 3));

        set_qty(&c, id, Some(0), -4).unwrap();
        let it = week_items(&mut c, MON, false).unwrap().remove(0);
        assert_eq!((it.target_qty, it.done_qty), (None, 0));
    }

    #[test]
    fn note_link_and_set_null_on_note_delete() {
        let mut c = db();
        c.execute("INSERT INTO notes (id, title, content) VALUES (1, 'Mitose', 'texto')", []).unwrap();
        c.execute("INSERT INTO tags (id, name) VALUES (1, 'bio')", []).unwrap();
        c.execute("INSERT INTO note_tags VALUES (1, 1)", []).unwrap();

        let id = create_item(&mut c, MON, "morning", "Bio", None).unwrap();
        link_note(&c, id, Some(1)).unwrap();
        assert!(link_note(&c, id, Some(42)).is_err()); // nota inexistente

        let it = week_items(&mut c, MON, false).unwrap().remove(0);
        assert_eq!(it.note_id, Some(1));

        assert_eq!(note_titles(&c).unwrap()[0].title, "Mitose");
        let d = note_detail(&c, 1).unwrap();
        assert_eq!((d.title.as_str(), d.tags.clone()), ("Mitose", vec!["bio".to_string()]));
        assert!(note_detail(&c, 2).is_err());

        c.execute("DELETE FROM notes WHERE id = 1", []).unwrap();
        let it = week_items(&mut c, MON, false).unwrap().remove(0);
        assert_eq!(it.note_id, None); // a tarefa sobrevive, só perde o vínculo
    }

    #[test]
    fn deleting_template_item_keeps_generated_tasks() {
        let mut c = db();
        let t = template_add(&c, 0, "morning", "Mat", None).unwrap();
        week_items(&mut c, MON, true).unwrap();
        template_remove(&c, t).unwrap();
        let it = week_items(&mut c, MON, false).unwrap();
        assert_eq!(it.len(), 1);
        assert_eq!(it[0].template_item_id, None);
        assert_eq!(template_sync(&mut c, MON).unwrap(), 0);
    }

    #[test]
    fn deleting_parent_cascades_to_children() {
        let mut c = db();
        let p = create_item(&mut c, MON, "morning", "P", None).unwrap();
        create_item(&mut c, "", "", "S", Some(p)).unwrap();
        remove_item(&mut c, p).unwrap();
        assert!(week_items(&mut c, MON, false).unwrap().is_empty());
    }
}

// ================================================================ TAURI COMMANDS
// (tudo abaixo só delega para as funções acima)

use crate::DbState;
use tauri::State;

fn with_db<T>(
    state: &State<'_, DbState>,
    f: impl FnOnce(&mut Connection) -> R<T>,
) -> Result<T, String> {
    let mut conn = state.inner().0.lock().map_err(|e| e.to_string())?;
    f(&mut conn).map_err(|e| e.0)
}

#[tauri::command]
pub fn get_week(
    state: State<'_, DbState>,
    week_start: String,
    generate: bool,
) -> Result<Vec<PlanItem>, String> {
    with_db(&state, |c| week_items(c, &week_start, generate))
}

#[tauri::command]
pub fn get_overdue(state: State<'_, DbState>, today: String) -> Result<Vec<PlanItem>, String> {
    with_db(&state, |c| overdue_items(c, &today))
}

#[tauri::command]
pub fn add_item(
    state: State<'_, DbState>,
    date: String,
    period: String,
    title: String,
    parent_id: Option<i64>,
) -> Result<i64, String> {
    with_db(&state, |c| create_item(c, &date, &period, &title, parent_id))
}

#[tauri::command]
pub fn toggle_item(state: State<'_, DbState>, id: i64, done: bool) -> Result<(), String> {
    with_db(&state, |c| set_done(c, id, done))
}

#[tauri::command]
pub fn rename_item(state: State<'_, DbState>, id: i64, title: String) -> Result<(), String> {
    with_db(&state, |c| rename(c, id, &title))
}

#[tauri::command]
pub fn set_item_qty(
    state: State<'_, DbState>,
    id: i64,
    target_qty: Option<i64>,
    done_qty: i64,
) -> Result<(), String> {
    with_db(&state, |c| set_qty(c, id, target_qty, done_qty))
}

#[tauri::command]
pub fn set_item_note(
    state: State<'_, DbState>,
    id: i64,
    note_id: Option<i64>,
) -> Result<(), String> {
    with_db(&state, |c| link_note(c, id, note_id))
}

#[tauri::command]
pub fn delete_item(state: State<'_, DbState>, id: i64) -> Result<(), String> {
    with_db(&state, |c| remove_item(c, id))
}

#[tauri::command]
pub fn move_item(
    state: State<'_, DbState>,
    id: i64,
    date: String,
    period: String,
    before_id: Option<i64>,
) -> Result<(), String> {
    with_db(&state, |c| relocate_item(c, id, &date, &period, before_id))
}

#[tauri::command]
pub fn list_template(state: State<'_, DbState>) -> Result<Vec<TemplateItem>, String> {
    with_db(&state, |c| template_list(c))
}

#[tauri::command]
pub fn add_template_item(
    state: State<'_, DbState>,
    weekday: i64,
    period: String,
    title: String,
    target_qty: Option<i64>,
) -> Result<i64, String> {
    with_db(&state, |c| template_add(c, weekday, &period, &title, target_qty))
}

#[tauri::command]
pub fn update_template_item(
    state: State<'_, DbState>,
    id: i64,
    title: String,
    target_qty: Option<i64>,
) -> Result<(), String> {
    with_db(&state, |c| template_update(c, id, &title, target_qty))
}

#[tauri::command]
pub fn delete_template_item(state: State<'_, DbState>, id: i64) -> Result<(), String> {
    with_db(&state, |c| template_remove(c, id))
}

#[tauri::command]
pub fn sync_template(state: State<'_, DbState>, week_start: String) -> Result<usize, String> {
    with_db(&state, |c| template_sync(c, &week_start))
}

#[tauri::command]
pub fn list_note_titles(state: State<'_, DbState>) -> Result<Vec<NoteTitle>, String> {
    with_db(&state, |c| note_titles(c))
}

#[tauri::command]
pub fn get_note(state: State<'_, DbState>, id: i64) -> Result<NoteDetail, String> {
    with_db(&state, |c| note_detail(c, id))
}
