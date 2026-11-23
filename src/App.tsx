import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { TagInput } from './components/TagInput';
import { Planner } from './components/Planner';

interface Note {
  id: number;
  title: string;
  content: string;
  tags: string[];
}

interface Tag {
  id: number;
  name: string;
  noteCount: number;
}

type View = 'notes' | 'planner';

const tabStyle = (active: boolean): React.CSSProperties => ({
  padding: '6px 16px',
  borderRadius: '8px',
  backgroundColor: active ? '#396cd8' : undefined,
  color: active ? '#fff' : undefined,
});

function App() {
  const [view, setView] = useState<View>('notes');

  const [title, setTitle] = useState('');
  const [content, setContent] = useState('');
  const [newTags, setNewTags] = useState<string[]>([]); // tags do formulário de criação
  const [editingNoteId, setEditingNoteId] = useState<number | null>(null);
  const [searchQuery, setSearchQuery] = useState('');

  const [notes, setNotes] = useState<Note[]>([]);
  const [selectedNote, setSelectedNote] = useState<Note | null>(null);

  const [allTags, setAllTags] = useState<Tag[]>([]);
  const [activeTag, setActiveTag] = useState<string | null>(null); // filtro por tag

  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Busca única: texto e tag são opcionais (vazio + null = todas as notas)
  const fetchNotes = async (query: string = searchQuery, tag: string | null = activeTag) => {
    try {
      const result = await invoke<Note[]>('search_notes', { query, tag });
      setNotes(result);
    } catch (err) {
      setError(err as string);
    }
  };

  const refreshTags = async () => {
    try {
      const result = await invoke<Tag[]>('list_tags');
      setAllTags(result);
      // Se a tag filtrada deixou de existir (ficou sem notas), volta para "Todas"
      setActiveTag((cur) => (cur && result.some((t) => t.name === cur) ? cur : null));
    } catch (err) {
      setError(err as string);
    }
  };

  useEffect(() => {
    refreshTags();
  }, []);

  // Recarrega a lista ao abrir o app e sempre que o filtro de tag mudar
  useEffect(() => {
    fetchNotes(searchQuery, activeTag);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeTag]);

  const handleSave = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!title || !content) return;

    setLoading(true);
    try {
      if (editingNoteId !== null) {
        await invoke('update_note', { id: editingNoteId, title, content });
      } else {
        await invoke('save_note', { title, content, tags: newTags });
      }
      setTitle('');
      setContent('');
      setNewTags([]);
      setEditingNoteId(null);
      await Promise.all([fetchNotes(), refreshTags()]);
    } catch (err) {
      setError(err as string);
    } finally {
      setLoading(false);
    }
  };

  const handleSearch = async () => {
    setLoading(true);
    await fetchNotes();
    setLoading(false);
  };

  const handleDelete = async (id: number, e: React.MouseEvent) => {
    e.stopPropagation();
    if (!confirm('Tem certeza que deseja apagar esta nota?')) return;

    try {
      await invoke('delete_note', { id });
      if (selectedNote?.id === id) setSelectedNote(null);
      await Promise.all([fetchNotes(), refreshTags()]);
    } catch (err) {
      setError(err as string);
    }
  };

  // Preenche o formulário com os dados da nota para edição
  const handleEdit = (note: Note, e: React.MouseEvent) => {
    e.stopPropagation();
    setTitle(note.title);
    setContent(note.content);
    setEditingNoteId(note.id);
  };

  // Cancela a edição e volta ao modo de criação
  const handleCancelEdit = () => {
    setTitle('');
    setContent('');
    setNewTags([]);
    setEditingNoteId(null);
  };

  // Edita as tags da nota que está aberta na coluna direita
  const handleTagsChange = async (next: string[]) => {
    if (!selectedNote) return;
    try {
      const saved = await invoke<string[]>('set_note_tags', {
        noteId: selectedNote.id,
        tags: next,
      });
      setSelectedNote({ ...selectedNote, tags: saved });
      await Promise.all([fetchNotes(), refreshTags()]);
    } catch (err) {
      setError(err as string);
    }
  };

  // Chamado pelo Planejador: abre a nota vinculada na aba de Notas
  const openNote = async (id: number) => {
    try {
      const note = await invoke<Note>('get_note', { id });
      setSelectedNote(note);
      setView('notes');
    } catch (err) {
      setError(err as string);
    }
  };

  return (
    <div style={{ fontFamily: 'sans-serif' }}>
      <nav style={{ display: 'flex', gap: '8px', padding: '12px 20px 0' }}>
        <button onClick={() => setView('notes')} style={tabStyle(view === 'notes')}>
          Notas
        </button>
        <button onClick={() => setView('planner')} style={tabStyle(view === 'planner')}>
          Planejador
        </button>
      </nav>

      {view === 'planner' ? (
        <Planner onOpenNote={openNote} />
      ) : (
        <div style={{ display: 'flex', padding: '20px', gap: '20px' }}>

          {/* COLUNA ESQUERDA: Formulário, Busca, Filtro de tags e Lista */}
          <div style={{ flex: 1, borderRight: '1px solid #ccc', paddingRight: '20px' }}>
            <h1>Plataforma de Estudos</h1>
            {error && <p style={{ color: 'red' }}>Erro: {error}</p>}

            <form onSubmit={handleSave} style={{ display: 'flex', flexDirection: 'column', gap: '10px', marginBottom: '20px' }}>
              <input value={title} onChange={(e) => setTitle(e.target.value)} placeholder="Título" style={{ padding: '8px' }} />
              <textarea value={content} onChange={(e) => setContent(e.target.value)} placeholder="Conteúdo..." style={{ padding: '8px', minHeight: '80px' }} />
              <TagInput tags={newTags} onChange={setNewTags} />
              <div style={{ display: 'flex', gap: '10px' }}>
                <button type="submit" disabled={loading} style={{ padding: '10px' }}>
                  {editingNoteId !== null ? 'Salvar Alterações' : 'Salvar Nota'}
                </button>
                {editingNoteId !== null && (
                  <button type="button" onClick={handleCancelEdit} style={{ padding: '10px' }}>
                    Cancelar Edição
                  </button>
                )}
              </div>
            </form>

            <div style={{ display: 'flex', gap: '10px', marginBottom: '12px' }}>
              <input value={searchQuery} onChange={(e) => setSearchQuery(e.target.value)} placeholder="Buscar..." style={{ padding: '8px', flex: 1 }} />
              <button onClick={handleSearch} disabled={loading} style={{ padding: '10px' }}>Buscar</button>
            </div>

            {/* Filtro por tag */}
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: '6px', marginBottom: '16px' }}>
              <button
                onClick={() => setActiveTag(null)}
                style={{
                  padding: '2px 10px', borderRadius: '999px',
                  backgroundColor: activeTag === null ? '#396cd8' : undefined,
                  color: activeTag === null ? '#fff' : undefined,
                }}
              >
                Todas
              </button>
              {allTags.map((t) => (
                <button
                  key={t.id}
                  onClick={() => setActiveTag(activeTag === t.name ? null : t.name)}
                  style={{
                    padding: '2px 10px', borderRadius: '999px',
                    backgroundColor: activeTag === t.name ? '#396cd8' : undefined,
                    color: activeTag === t.name ? '#fff' : undefined,
                  }}
                >
                  #{t.name} <small>({t.noteCount})</small>
                </button>
              ))}
            </div>

            <ul style={{ listStyleType: 'none', padding: 0 }}>
              {notes.map((note) => (
                <li
                  key={note.id}
                  onClick={() => setSelectedNote(note)}
                  style={{
                    padding: '10px',
                    border: '1px solid #ddd',
                    marginBottom: '5px',
                    cursor: 'pointer',
                    backgroundColor: selectedNote?.id === note.id ? '#e0f7fa' : '#fff',
                    display: 'flex',
                    justifyContent: 'space-between',
                    gap: '10px',
                  }}
                >
                  <span>
                    <strong>{note.title}</strong>
                    {note.tags.length > 0 && (
                      <div style={{ display: 'flex', flexWrap: 'wrap', gap: '4px', marginTop: '4px' }}>
                        {note.tags.map((t) => (
                          <span key={t} style={{ fontSize: '0.75em', color: '#1a3a8a', backgroundColor: '#e8eefc', borderRadius: '999px', padding: '0 8px' }}>
                            #{t}
                          </span>
                        ))}
                      </div>
                    )}
                  </span>
                  <div style={{ display: 'flex', gap: '6px', alignSelf: 'flex-start' }}>
                    <button
                      onClick={(e) => handleEdit(note, e)}
                      style={{ backgroundColor: '#cfe2ff', color: '#1a3a8a', border: 'none', borderRadius: '4px', cursor: 'pointer', padding: '2px 8px' }}
                    >
                      Editar
                    </button>
                    <button
                      onClick={(e) => handleDelete(note.id, e)}
                      style={{ backgroundColor: '#ffcccc', color: 'red', border: 'none', borderRadius: '4px', cursor: 'pointer', padding: '2px 8px' }}
                    >
                      Apagar
                    </button>
                  </div>
                </li>
              ))}
            </ul>
          </div>

          {/* COLUNA DIREITA: Visualização do Conteúdo + edição de tags */}
          <div style={{ flex: 1 }}>
            {selectedNote ? (
              <div>
                <h2>{selectedNote.title}</h2>
                <div style={{ marginBottom: '12px' }}>
                  <TagInput tags={selectedNote.tags} onChange={handleTagsChange} />
                </div>
                <p style={{ whiteSpace: 'pre-wrap', lineHeight: '1.6' }}>{selectedNote.content}</p>
              </div>
            ) : (
              <p style={{ color: '#888' }}>Selecione uma nota na lista para visualizar o conteúdo.</p>
            )}
          </div>

        </div>
      )}
    </div>
  );
}

export default App;

