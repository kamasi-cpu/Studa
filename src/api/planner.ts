import { invoke } from '@tauri-apps/api/core';

export type Period = 'morning' | 'afternoon' | 'evening';

export interface PlanItem {
  id: number;
  date: string; // 'AAAA-MM-DD'
  period: Period;
  title: string;
  position: number;
  parentId: number | null;
  doneAt: string | null; // null = pendente
  templateItemId: number | null;
  noteId: number | null;
  targetQty: number | null;
  doneQty: number;
}

export interface TemplateItem {
  id: number;
  weekday: number; // 0 = segunda
  period: Period;
  title: string;
  targetQty: number | null;
  position: number;
}

export interface NoteTitle {
  id: number;
  title: string;
}

export interface NoteDetail {
  id: number;
  title: string;
  content: string;
  tags: string[];
}

// ---- semana / tarefas
export const getWeek = (weekStart: string, generate: boolean) =>
  invoke<PlanItem[]>('get_week', { weekStart, generate });

export const getOverdue = (today: string) => invoke<PlanItem[]>('get_overdue', { today });

export const addItem = (
  date: string,
  period: Period,
  title: string,
  parentId: number | null = null,
) => invoke<number>('add_item', { date, period, title, parentId });

export const toggleItem = (id: number, done: boolean) => invoke<void>('toggle_item', { id, done });

export const renameItem = (id: number, title: string) => invoke<void>('rename_item', { id, title });

export const setItemQty = (id: number, targetQty: number | null, doneQty: number) =>
  invoke<void>('set_item_qty', { id, targetQty, doneQty });

export const setItemNote = (id: number, noteId: number | null) =>
  invoke<void>('set_item_note', { id, noteId });

export const deleteItem = (id: number) => invoke<void>('delete_item', { id });

export const moveItem = (id: number, date: string, period: Period, beforeId: number | null) =>
  invoke<void>('move_item', { id, date, period, beforeId });

// ---- rotina-modelo
export const listTemplate = () => invoke<TemplateItem[]>('list_template');

export const addTemplateItem = (
  weekday: number,
  period: Period,
  title: string,
  targetQty: number | null,
) => invoke<number>('add_template_item', { weekday, period, title, targetQty });

export const updateTemplateItem = (id: number, title: string, targetQty: number | null) =>
  invoke<void>('update_template_item', { id, title, targetQty });

export const deleteTemplateItem = (id: number) => invoke<void>('delete_template_item', { id });

export const syncTemplate = (weekStart: string) => invoke<number>('sync_template', { weekStart });

// ---- notas (para o vínculo)
export const listNoteTitles = () => invoke<NoteTitle[]>('list_note_titles');

export const getNote = (id: number) => invoke<NoteDetail>('get_note', { id });
