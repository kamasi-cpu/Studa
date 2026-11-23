import { useState } from "react";

export default function NoteEditor() {
  const [note, setNote] = useState("");

  const handleSave = () => {
    // Por enquanto, apenas exibe o conteúdo no console.
    console.log(note);
  };

  return (
    <div>
      <textarea
        aria-label="Nota"
        placeholder="Escreva sua nota..."
        rows={6}
        value={note}
        onChange={(e) => setNote(e.target.value)}
      />
      <button type="button" onClick={handleSave}>
        Salvar
      </button>
    </div>
  );
}
