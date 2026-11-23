import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import * as api from '../api/planner';
import type { NoteTitle, Period, PlanItem } from '../api/planner';
import { addDays, formatDM, mondayOf, todayISO, weekdayIndex, WEEKDAYS } from '../lib/dates';
import { TemplateEditor } from './TemplateEditor';
import './planner.css';

const PERIODS: { key: Period; label: string }[] = [
  { key: 'morning', label: 'MANHÃ' },
  { key: 'afternoon', label: 'TARDE' },
];

const periodName = (p: Period) => (p === 'morning' ? 'manhã' : p === 'afternoon' ? 'tarde' : 'noite');
const cellKey = (date: string, period: string) => `${date}|${period}`;
const parseIntOrNull = (s: string) => {
  const n = parseInt(s, 10);
  return Number.isFinite(n) ? n : null;
};

// ------------------------------------------------------------------ peças pequenas

function ProgressBar({ done, total, showText = true }: { done: number; total: number; showText?: boolean }) {
  const pct = total > 0 ? Math.min(100, Math.round((done / total) * 100)) : 0;
  return (
    <div className="pl-progress">
      <div className="pl-bar">
        <div className="pl-bar-fill" style={{ width: `${pct}%` }} />
      </div>
      {showText && (
        <span className="pl-progress-text">
          {done}/{total}
        </span>
      )}
    </div>
  );
}

function AddInline({
  label,
  placeholder,
  onAdd,
  defaultOpen = false,
}: {
  label: string;
  placeholder: string;
  onAdd: (title: string) => void;
  defaultOpen?: boolean;
}) {
  const [open, setOpen] = useState(defaultOpen);
  const [text, setText] = useState('');
  const cancelled = useRef(false);

  if (!open) {
    return (
      <button
        className="pl-add"
        onClick={() => {
          cancelled.current = false;
          setOpen(true);
        }}
      >
        {label}
      </button>
    );
  }

  const commit = () => {
    const t = text.trim();
    if (t) onAdd(t);
    setText('');
  };

  return (
    <input
      autoFocus
      className="pl-add-input"
      value={text}
      placeholder={placeholder}
      onChange={(e) => setText(e.target.value)}
      onKeyDown={(e) => {
        if (e.key === 'Enter') commit();
        else if (e.key === 'Escape') {
          cancelled.current = true;
          setText('');
          setOpen(false);
        }
      }}
      onBlur={() => {
        if (!cancelled.current) commit();
        setOpen(false);
      }}
    />
  );
}

// ------------------------------------------------------------------ linha de tarefa

interface Actions {
  toggle: (it: PlanItem) => void;
  rename: (it: PlanItem, title: string) => void;
  setQty: (it: PlanItem, target: number | null, done: number) => void;
  setNote: (it: PlanItem, noteId: number | null) => void;
  remove: (it: PlanItem, hasKids: boolean) => void;
  addSub: (parent: PlanItem, title: string) => void;
  openNote: (noteId: number) => void;
}

interface DragProps {
  start: (id: number) => void;
  end: () => void;
  over: () => void;
  drop: (beforeId: number) => void;
}

function ItemRow({
  item,
  kids,
  notes,
  today,
  actions,
  drag,
}: {
  item: PlanItem;
  kids: PlanItem[];
  notes: NoteTitle[];
  today: string;
  actions: Actions;
  drag?: DragProps; // só tarefas de nível superior são arrastáveis
}) {
  const [panel, setPanel] = useState<null | 'qty' | 'note' | 'sub'>(null);
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(item.title);
  const [targetStr, setTargetStr] = useState('');
  const [doneStr, setDoneStr] = useState('');
  const cancelEdit = useRef(false);

  const isChild = item.parentId !== null;
  const done = item.doneAt !== null;
  const late = !isChild && !done && item.date < today;
  const linkedNoteId = item.noteId;
  const linkedNote = linkedNoteId !== null ? notes.find((n) => n.id === linkedNoteId) : undefined;

  const commitTitle = () => {
    setEditing(false);
    const t = draft.trim();
    if (t && t !== item.title) actions.rename(item, t);
  };

  const togglePanel = (p: 'qty' | 'note' | 'sub') => {
    if (p === 'qty') {
      setTargetStr(item.targetQty?.toString() ?? '');
      setDoneStr(String(item.doneQty));
    }
    setPanel((cur) => (cur === p ? null : p));
  };

  return (
    <li
      className={`pl-item${late ? ' pl-late' : ''}${isChild ? ' pl-child' : ''}`}
      draggable={!!drag && !editing && panel === null}
      onDragStart={
        drag
          ? (e) => {
              e.stopPropagation();
              e.dataTransfer.setData('text/plain', String(item.id));
              e.dataTransfer.effectAllowed = 'move';
              drag.start(item.id);
            }
          : undefined
      }
      onDragEnd={drag ? drag.end : undefined}
      onDragOver={
        drag
          ? (e) => {
              e.preventDefault();
              e.stopPropagation();
              drag.over();
            }
          : undefined
      }
      onDrop={
        drag
          ? (e) => {
              e.preventDefault();
              e.stopPropagation();
              drag.drop(item.id);
            }
          : undefined
      }
    >
      <div className="pl-row">
        <input
          type="checkbox"
          checked={done}
          onChange={() => actions.toggle(item)}
          aria-label={`Concluir ${item.title}`}
        />

        {editing ? (
          <input
            autoFocus
            className="pl-title-input"
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onBlur={() => {
              if (cancelEdit.current) {
                cancelEdit.current = false;
                setEditing(false);
              } else {
                commitTitle();
              }
            }}
            onKeyDown={(e) => {
              if (e.key === 'Enter') commitTitle();
              else if (e.key === 'Escape') {
                cancelEdit.current = true;
                setEditing(false);
              }
            }}
          />
        ) : (
          <span
            className={`pl-title${done ? ' pl-done' : ''}`}
            title="Duplo clique para editar"
            onDoubleClick={() => {
              setDraft(item.title);
              setEditing(true);
            }}
          >
            {item.title}
          </span>
        )}

        {item.targetQty !== null && (
          <span className="pl-qty">
            <button
              aria-label="Diminuir"
              onClick={() => actions.setQty(item, item.targetQty, Math.max(0, item.doneQty - 1))}
            >
              −
            </button>
            <span>
              {item.doneQty}/{item.targetQty}
            </span>
            <button aria-label="Aumentar" onClick={() => actions.setQty(item, item.targetQty, item.doneQty + 1)}>
              +
            </button>
          </span>
        )}

        {linkedNoteId !== null && (
          <button
            className="pl-chip"
            title="Abrir nota vinculada"
            onClick={() => actions.openNote(linkedNoteId)}
          >
            📎 {linkedNote ? linkedNote.title : 'nota'}
          </button>
        )}

        <span className="pl-actions">
          <button title="Meta de questões" aria-label="Definir meta" onClick={() => togglePanel('qty')}>
            🎯
          </button>
          <button title="Vincular nota" aria-label="Vincular nota" onClick={() => togglePanel('note')}>
            📎
          </button>
          {!isChild && (
            <button title="Adicionar subitem" aria-label="Adicionar subitem" onClick={() => togglePanel('sub')}>
              ＋
            </button>
          )}
          <button
            title="Apagar"
            aria-label={`Apagar ${item.title}`}
            onClick={() => actions.remove(item, kids.length > 0)}
          >
            ×
          </button>
        </span>
      </div>

      {panel === 'qty' && (
        <div className="pl-panel">
          <label>
            Meta
            <input type="number" min={0} value={targetStr} onChange={(e) => setTargetStr(e.target.value)} />
          </label>
          <label>
            Feitas
            <input type="number" min={0} value={doneStr} onChange={(e) => setDoneStr(e.target.value)} />
          </label>
          <button
            onClick={() => {
              actions.setQty(item, parseIntOrNull(targetStr), parseIntOrNull(doneStr) ?? 0);
              setPanel(null);
            }}
          >
            Salvar
          </button>
          {item.targetQty !== null && (
            <button
              onClick={() => {
                actions.setQty(item, null, 0);
                setPanel(null);
              }}
            >
              Remover meta
            </button>
          )}
        </div>
      )}

      {panel === 'note' && (
        <div className="pl-panel">
          <select
            value={item.noteId ?? ''}
            onChange={(e) => {
              actions.setNote(item, e.target.value === '' ? null : Number(e.target.value));
              setPanel(null);
            }}
          >
            <option value="">Sem vínculo</option>
            {notes.map((n) => (
              <option key={n.id} value={n.id}>
                {n.title}
              </option>
            ))}
          </select>
        </div>
      )}

      {panel === 'sub' && (
        <div className="pl-panel">
          <AddInline
            label="+ subitem"
            placeholder="Ex.: 5q. Embriologia"
            defaultOpen
            onAdd={(t) => actions.addSub(item, t)}
          />
        </div>
      )}

      {kids.length > 0 && (
        <ul className="pl-kids">
          {kids.map((k) => (
            <ItemRow key={k.id} item={k} kids={[]} notes={notes} today={today} actions={actions} />
          ))}
        </ul>
      )}
    </li>
  );
}

// ------------------------------------------------------------------ componente principal

export function Planner({ onOpenNote }: { onOpenNote: (noteId: number) => void }) {
  const today = todayISO();
  const currentWeek = mondayOf(today);

  const [weekStart, setWeekStart] = useState(currentWeek);
  const [items, setItems] = useState<PlanItem[]>([]);
  const [overdue, setOverdue] = useState<PlanItem[]>([]);
  const [notes, setNotes] = useState<NoteTitle[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [showRoutine, setShowRoutine] = useState(false);
  const [showOverdue, setShowOverdue] = useState(true);
  const [dragId, setDragId] = useState<number | null>(null);
  const [dropKey, setDropKey] = useState<string | null>(null);

  const days = useMemo(() => [0, 1, 2, 3, 4].map((i) => addDays(weekStart, i)), [weekStart]);

  const reload = useCallback(async () => {
    try {
      // Só semanas atual/futuras são geradas a partir da rotina (passado fica como está).
      const week = await api.getWeek(weekStart, weekStart >= currentWeek);
      const late = await api.getOverdue(today); // depois, para já enxergar a semana gerada
      setItems(week);
      setOverdue(late);
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, [weekStart, currentWeek, today]);

  useEffect(() => {
    reload();
  }, [reload]);

  useEffect(() => {
    api.listNoteTitles().then(setNotes).catch((e) => setError(String(e)));
  }, []);

  const act = async (fn: () => Promise<unknown>) => {
    try {
      await fn();
      await reload();
    } catch (e) {
      setError(String(e));
    }
  };

  // ---- derivados
  const { cells, kids } = useMemo(() => {
    const cells = new Map<string, PlanItem[]>();
    const kids = new Map<number, PlanItem[]>();
    for (const it of items) {
      if (it.parentId !== null) {
        const list = kids.get(it.parentId) ?? [];
        list.push(it);
        kids.set(it.parentId, list);
      } else {
        const k = cellKey(it.date, it.period);
        const list = cells.get(k) ?? [];
        list.push(it);
        cells.set(k, list);
      }
    }
    return { cells, kids };
  }, [items]);

  const visible = useMemo(() => items.filter((i) => days.includes(i.date)), [items, days]);
  const tops = useMemo(() => visible.filter((i) => i.parentId === null), [visible]);

  const dayStats = (date: string) => {
    const t = tops.filter((i) => i.date === date);
    return { done: t.filter((i) => i.doneAt !== null).length, total: t.length };
  };
  const weekStats = {
    done: tops.filter((i) => i.doneAt !== null).length,
    total: tops.length,
  };

  // Metas da semana por disciplina (subitem soma na disciplina da tarefa-pai)
  const subjects = useMemo(() => {
    const byId = new Map(visible.map((i) => [i.id, i]));
    const map = new Map<string, { name: string; target: number; done: number }>();
    for (const it of visible) {
      if (it.targetQty === null) continue;
      const owner = it.parentId !== null ? byId.get(it.parentId) ?? it : it;
      const key = owner.title.trim().toLowerCase();
      const cur = map.get(key) ?? { name: owner.title, target: 0, done: 0 };
      cur.target += it.targetQty;
      cur.done += it.doneQty;
      map.set(key, cur);
    }
    return [...map.values()].sort((a, b) => a.name.localeCompare(b.name));
  }, [visible]);

  // "Mover para hoje" (ou para a próxima segunda, se hoje é fim de semana)
  const moveTarget = weekdayIndex(today) < 5 ? today : addDays(mondayOf(today), 7);
  const moveLabel = weekdayIndex(today) < 5 ? 'hoje' : 'segunda';

  // ---- ações
  const actions: Actions = {
    toggle: (it) => act(() => api.toggleItem(it.id, it.doneAt === null)),
    rename: (it, title) => act(() => api.renameItem(it.id, title)),
    setQty: (it, target, done) => act(() => api.setItemQty(it.id, target, done)),
    setNote: (it, noteId) => act(() => api.setItemNote(it.id, noteId)),
    remove: (it, hasKids) => {
      if (hasKids && !confirm(`Apagar "${it.title}" e seus subitens?`)) return;
      act(() => api.deleteItem(it.id));
    },
    addSub: (parent, title) => act(() => api.addItem(parent.date, parent.period, title, parent.id)),
    openNote: onOpenNote,
  };

  const endDrag = () => {
    setDragId(null);
    setDropKey(null);
  };

  const handleDrop = (date: string, period: Period, beforeId: number | null) => {
    const id = dragId;
    endDrag();
    if (id === null || id === beforeId) return;
    act(() => api.moveItem(id, date, period, beforeId));
  };

  return (
    <div className="pl-root">
      <header className="pl-toolbar">
        <button onClick={() => setWeekStart(addDays(weekStart, -7))} aria-label="Semana anterior">
          ◀
        </button>
        <strong className="pl-range">
          {formatDM(days[0])} – {formatDM(days[4])}
          <small> / {days[4].slice(0, 4)}</small>
        </strong>
        <button onClick={() => setWeekStart(addDays(weekStart, 7))} aria-label="Próxima semana">
          ▶
        </button>
        <button onClick={() => setWeekStart(currentWeek)} disabled={weekStart === currentWeek}>
          Esta semana
        </button>
        <span className="pl-spacer" />
        <div className="pl-week-progress" title="Tarefas concluídas na semana">
          <span>Semana</span>
          <ProgressBar done={weekStats.done} total={weekStats.total} />
        </div>
        <button onClick={() => setShowRoutine(true)}>Rotina</button>
      </header>

      {error && <p className="pl-error">Erro: {error}</p>}

      {overdue.length > 0 && (
        <section className="pl-overdue">
          <button className="pl-overdue-head" onClick={() => setShowOverdue((v) => !v)}>
            ⚠ Em atraso ({overdue.length}) {showOverdue ? '▾' : '▸'}
          </button>
          {showOverdue && (
            <ul>
              {overdue.map((o) => (
                <li
                  key={o.id}
                  draggable
                  onDragStart={(e) => {
                    e.dataTransfer.setData('text/plain', String(o.id));
                    e.dataTransfer.effectAllowed = 'move';
                    setDragId(o.id);
                  }}
                  onDragEnd={endDrag}
                >
                  <input
                    type="checkbox"
                    checked={false}
                    onChange={() => act(() => api.toggleItem(o.id, true))}
                    aria-label={`Concluir ${o.title}`}
                  />
                  <span className="pl-title">{o.title}</span>
                  <small>
                    {WEEKDAYS[weekdayIndex(o.date)]} {formatDM(o.date)} · {periodName(o.period)}
                  </small>
                  <button onClick={() => act(() => api.moveItem(o.id, moveTarget, o.period, null))}>
                    Mover para {moveLabel}
                  </button>
                  <button aria-label={`Apagar ${o.title}`} onClick={() => act(() => api.deleteItem(o.id))}>
                    ×
                  </button>
                </li>
              ))}
            </ul>
          )}
        </section>
      )}

      <div className="pl-grid">
        {days.map((date, i) => {
          const st = dayStats(date);
          return (
            <section key={date} className={`pl-day${date === today ? ' pl-today' : ''}`}>
              <h3>
                {WEEKDAYS[i]} <small>{formatDM(date)}</small>
              </h3>
              <ProgressBar done={st.done} total={st.total} />

              {PERIODS.map((p) => {
                const key = cellKey(date, p.key);
                return (
                  <div
                    key={key}
                    className={`pl-cell${dropKey === key ? ' pl-drop' : ''}`}
                    onDragOver={(e) => {
                      if (dragId === null) return;
                      e.preventDefault();
                      if (dropKey !== key) setDropKey(key);
                    }}
                    onDrop={(e) => {
                      e.preventDefault();
                      handleDrop(date, p.key, null);
                    }}
                  >
                    <h4>{p.label}</h4>
                    <ul>
                      {(cells.get(key) ?? []).map((it) => (
                        <ItemRow
                          key={it.id}
                          item={it}
                          kids={kids.get(it.id) ?? []}
                          notes={notes}
                          today={today}
                          actions={actions}
                          drag={{
                            start: setDragId,
                            end: endDrag,
                            over: () => {
                              if (dropKey !== key) setDropKey(key);
                            },
                            drop: (beforeId) => handleDrop(date, p.key, beforeId),
                          }}
                        />
                      ))}
                    </ul>
                    <AddInline
                      label="+ Adicionar"
                      placeholder="Nova tarefa…"
                      onAdd={(title) => act(() => api.addItem(date, p.key, title))}
                    />
                  </div>
                );
              })}
            </section>
          );
        })}
      </div>

      {subjects.length > 0 && (
        <section className="pl-goals">
          <h3>Metas da semana</h3>
          {subjects.map((s) => (
            <div key={s.name} className="pl-goal">
              <span className="pl-goal-name">{s.name}</span>
              <ProgressBar done={s.done} total={s.target} />
            </div>
          ))}
        </section>
      )}

      {showRoutine && (
        <TemplateEditor weekStart={weekStart} onChanged={reload} onClose={() => setShowRoutine(false)} />
      )}
    </div>
  );
}
