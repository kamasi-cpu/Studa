import { useCallback, useEffect, useState } from 'react';
import * as api from '../api/planner';
import type { Period, TemplateItem } from '../api/planner';
import { WEEKDAYS } from '../lib/dates';

const PERIODS: { key: Period; label: string }[] = [
  { key: 'morning', label: 'MANHÃ' },
  { key: 'afternoon', label: 'TARDE' },
];

const qtyOrNull = (s: string) => {
  const n = parseInt(s, 10);
  return Number.isFinite(n) && n > 0 ? n : null;
};

function TemplateRow({
  item,
  onSave,
  onDelete,
}: {
  item: TemplateItem;
  onSave: (id: number, title: string, target: number | null) => void;
  onDelete: (id: number) => void;
}) {
  const [title, setTitle] = useState(item.title);
  const [target, setTarget] = useState(item.targetQty?.toString() ?? '');

  useEffect(() => {
    setTitle(item.title);
    setTarget(item.targetQty?.toString() ?? '');
  }, [item.title, item.targetQty]);

  const commit = () => {
    const t = title.trim();
    if (!t) {
      setTitle(item.title);
      return;
    }
    const q = qtyOrNull(target);
    if (t !== item.title || q !== item.targetQty) onSave(item.id, t, q);
  };

  return (
    <li className="pl-trow">
      <input
        className="pl-t-title"
        value={title}
        onChange={(e) => setTitle(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => {
          if (e.key === 'Enter') e.currentTarget.blur();
        }}
      />
      <input
        className="pl-t-qty"
        type="number"
        min={0}
        placeholder="meta"
        title="Meta de questões (opcional)"
        value={target}
        onChange={(e) => setTarget(e.target.value)}
        onBlur={commit}
      />
      <button aria-label={`Remover ${item.title} da rotina`} onClick={() => onDelete(item.id)}>
        ×
      </button>
    </li>
  );
}

function AddTemplateItem({ onAdd }: { onAdd: (title: string, target: number | null) => void }) {
  const [title, setTitle] = useState('');
  const [target, setTarget] = useState('');

  const submit = () => {
    const t = title.trim();
    if (!t) return;
    onAdd(t, qtyOrNull(target));
    setTitle('');
    setTarget('');
  };

  return (
    <div className="pl-trow pl-tadd">
      <input
        className="pl-t-title"
        value={title}
        placeholder="Nova tarefa…"
        onChange={(e) => setTitle(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === 'Enter') submit();
        }}
      />
      <input
        className="pl-t-qty"
        type="number"
        min={0}
        placeholder="meta"
        value={target}
        onChange={(e) => setTarget(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === 'Enter') submit();
        }}
      />
      <button onClick={submit}>＋</button>
    </div>
  );
}

export function TemplateEditor({
  weekStart,
  onChanged,
  onClose,
}: {
  weekStart: string;
  onChanged: () => void | Promise<void>;
  onClose: () => void;
}) {
  const [items, setItems] = useState<TemplateItem[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      setItems(await api.listTemplate());
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const mutate = async (fn: () => Promise<unknown>) => {
    setMessage(null);
    try {
      await fn();
      setError(null);
      await load();
      await onChanged();
    } catch (e) {
      setError(String(e));
    }
  };

  const apply = () =>
    mutate(async () => {
      const n = await api.syncTemplate(weekStart);
      setMessage(n > 0 ? `${n} tarefa(s) adicionada(s) à semana.` : 'A semana já está completa.');
    });

  return (
    <div className="pl-modal" onClick={onClose}>
      <div className="pl-modal-card" onClick={(e) => e.stopPropagation()}>
        <header className="pl-modal-head">
          <h2>Rotina semanal</h2>
          <span className="pl-spacer" />
          <button onClick={apply} title="Adiciona à semana exibida o que ainda não está nela">
            Aplicar à semana exibida
          </button>
          <button onClick={onClose}>Fechar</button>
        </header>

        <p className="pl-hint">
          A rotina é o modelo: toda semana nova nasce dela. Editar a rotina não altera semanas já geradas — use
          “Aplicar à semana exibida” para incluir o que faltar.
        </p>
        {error && <p className="pl-error">Erro: {error}</p>}
        {message && <p className="pl-ok">{message}</p>}

        <div className="pl-tgrid">
          {WEEKDAYS.slice(0, 5).map((name, weekday) => (
            <section key={name} className="pl-tday">
              <h3>{name}</h3>
              {PERIODS.map((p) => (
                <div key={p.key} className="pl-tcell">
                  <h4>{p.label}</h4>
                  <ul>
                    {items
                      .filter((i) => i.weekday === weekday && i.period === p.key)
                      .map((i) => (
                        <TemplateRow
                          key={i.id}
                          item={i}
                          onSave={(id, title, target) => mutate(() => api.updateTemplateItem(id, title, target))}
                          onDelete={(id) => mutate(() => api.deleteTemplateItem(id))}
                        />
                      ))}
                  </ul>
                  <AddTemplateItem
                    onAdd={(title, target) => mutate(() => api.addTemplateItem(weekday, p.key, title, target))}
                  />
                </div>
              ))}
            </section>
          ))}
        </div>
      </div>
    </div>
  );
}
