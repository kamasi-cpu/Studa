import { useState } from 'react';

interface Props {
  tags: string[];
  onChange: (tags: string[]) => void;
  placeholder?: string;
}

export function TagInput({ tags, onChange, placeholder = 'Adicionar tag e Enter…' }: Props) {
  const [draft, setDraft] = useState('');

  const commit = () => {
    const name = draft.trim().replace(/^#/, '').toLowerCase();
    if (name && !tags.includes(name)) onChange([...tags, name]);
    setDraft('');
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter' || e.key === ',') {
      e.preventDefault(); // evita submeter o formulário de criação
      commit();
    } else if (e.key === 'Backspace' && !draft && tags.length > 0) {
      onChange(tags.slice(0, -1));
    }
  };

  return (
    <div style={{ display: 'flex', flexWrap: 'wrap', gap: '6px', alignItems: 'center' }}>
      {tags.map((t) => (
        <span
          key={t}
          style={{
            display: 'inline-flex', alignItems: 'center', gap: '4px',
            padding: '2px 8px', borderRadius: '999px',
            backgroundColor: '#e8eefc', color: '#1a3a8a', fontSize: '0.85em',
          }}
        >
          #{t}
          <button
            type="button"
            aria-label={`Remover tag ${t}`}
            onClick={() => onChange(tags.filter((x) => x !== t))}
            style={{
              border: 'none', background: 'none', cursor: 'pointer',
              padding: 0, boxShadow: 'none', color: 'inherit',
            }}
          >
            ×
          </button>
        </span>
      ))}
      <input
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        onKeyDown={handleKeyDown}
        onBlur={commit}
        placeholder={placeholder}
        style={{ padding: '6px', flex: 1, minWidth: '140px' }}
      />
    </div>
  );
}
