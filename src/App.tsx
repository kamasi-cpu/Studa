import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';

// Interface para tipar a Nota
interface Note {
  id: number;
  title: string;
  content: string;
}

function App() {
  const [title, setTitle] = useState('');
  const [content, setContent] = useState('');
  const [searchQuery, setSearchQuery] = useState('');
  
  // Estados das Notas
  const [notes, setNotes] = useState<Note[]>([]);
  const [selectedNote, setSelectedNote] = useState<Note | null>(null); // Nota em visualização
  
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Carrega todas as notas ao abrir o aplicativo
  const loadNotes = async () => {
    try {
      const result = await invoke<Note[]>('get_all_notes');
      setNotes(result);
    } catch (err) {
      setError(err as string);
    }
  };

  useEffect(() => {
    loadNotes();
  }, []);

  const handleSave = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!title || !content) return;
    
    setLoading(true);
    try {
      await invoke('save_note', { title, content });
      setTitle('');
      setContent('');
      await loadNotes(); // Recarrega a lista após salvar
    } catch (err) {
      setError(err as string);
    } finally {
      setLoading(false);
    }
  };

  const handleSearch = async () => {
    if (!searchQuery.trim()) {
      loadNotes(); // Se a busca estiver vazia, mostra tudo
      return;
    }
    setLoading(true);
    try {
      const result = await invoke<Note[]>('search_notes', { query: searchQuery });
      setNotes(result);
    } catch (err) {
      setError(err as string);
    } finally {
      setLoading(false);
    }
  };

  const handleDelete = async (id: number, e: React.MouseEvent) => {
    e.stopPropagation(); // Impede que o clique selecione a nota ao deletar
    if (!confirm('Tem certeza que deseja apagar esta nota?')) return;

    try {
      await invoke('delete_note', { id });
      if (selectedNote?.id === id) setSelectedNote(null); // Limpa a tela se a nota deletada estava aberta
      await loadNotes(); // Recarrega a lista
    } catch (err) {
      setError(err as string);
    }
  };

  return (
    <div style={{ display: 'flex', padding: '20px', fontFamily: 'sans-serif', gap: '20px' }}>
      
      {/* COLUNA ESQUERDA: Formulário, Busca e Lista */}
      <div style={{ flex: 1, borderRight: '1px solid #ccc', paddingRight: '20px' }}>
        <h1>Plataforma de Estudos</h1>
        {error && <p style={{ color: 'red' }}>Erro: {error}</p>}

        <form onSubmit={handleSave} style={{ display: 'flex', flexDirection: 'column', gap: '10px', marginBottom: '20px' }}>
          <input value={title} onChange={(e) => setTitle(e.target.value)} placeholder="Título" style={{ padding: '8px' }} />
          <textarea value={content} onChange={(e) => setContent(e.target.value)} placeholder="Conteúdo..." style={{ padding: '8px', minHeight: '80px' }} />
          <button type="submit" disabled={loading} style={{ padding: '10px' }}>Salvar Nota</button>
        </form>

        <div style={{ display: 'flex', gap: '10px', marginBottom: '20px' }}>
          <input value={searchQuery} onChange={(e) => setSearchQuery(e.target.value)} placeholder="Buscar..." style={{ padding: '8px', flex: 1 }} />
          <button onClick={handleSearch} disabled={loading} style={{ padding: '10px' }}>Buscar</button>
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
                justifyContent: 'space-between' 
              }}
            >
              <span><strong>{note.title}</strong></span>
              <button 
                onClick={(e) => handleDelete(note.id, e)}
                style={{ backgroundColor: '#ffcccc', color: 'red', border: 'none', borderRadius: '4px', cursor: 'pointer', padding: '2px 8px' }}
              >
                Apagar
              </button>
            </li>
          ))}
        </ul>
      </div>

      {/* COLUNA DIREITA: Visualização do Conteúdo */}
      <div style={{ flex: 1 }}>
        {selectedNote ? (
          <div>
            <h2>{selectedNote.title}</h2>
            <p style={{ whiteSpace: 'pre-wrap', lineHeight: '1.6' }}>{selectedNote.content}</p>
          </div>
        ) : (
          <p style={{ color: '#888' }}>Selecione uma nota na lista para visualizar o conteúdo.</p>
        )}
      </div>

    </div>
  );
}

export default App;