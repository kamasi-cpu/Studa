# 🗺️ Roadmap e Ideias Futuras

## ✅ Concluído
- [x] Estrutura base (Tauri + React + SQLite)
- [x] CRUD básico de Notas (Criar, Ler, Deletar)
- [ ] Edição de Notas (Em andamento)

## 💡 Ideias Futuras (Backlog)

### 🧠 Sistema de Flashcards e Repetição Espaçada (SRS)
- **Objetivo**: Adicionar suporte a flashcards para memorização ativa.
- **Opções de Implementação (Avaliadas)**:
  1. **Integração com Anki Desktop (Recomendado)**: Usar o add-on *AnkiConnect* via API HTTP local. No backend Rust, usar as crates `ankit` ou `ankiconnect-rs` para criar/ler cards diretamente na coleção local do usuário.
  2. **SRS Interno**: Implementar o algoritmo FSRS ou SM-2 nativamente no backend Rust e criar tabelas próprias no SQLite.
  3. **Import/Export `.apkg`**: Criar rotinas para gerar e ler arquivos ZIP com o banco SQLite do Anki.
  4. **Evitar**: Sincronização direta com a nuvem do AnkiWeb (protocolo fechado e frágil).
  