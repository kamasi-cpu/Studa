// Datas como texto 'AAAA-MM-DD' no fuso LOCAL (evita bugs de UTC com toISOString()).

const pad = (n: number) => String(n).padStart(2, '0');

export function toISO(d: Date): string {
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

export function parseISO(iso: string): Date {
  const [y, m, d] = iso.split('-').map(Number);
  return new Date(y, m - 1, d);
}

export function addDays(iso: string, n: number): string {
  const d = parseISO(iso);
  d.setDate(d.getDate() + n); // setDate lida com virada de mês/ano e horário de verão
  return toISO(d);
}

export function todayISO(): string {
  return toISO(new Date());
}

/** 0 = segunda ... 6 = domingo */
export function weekdayIndex(iso: string): number {
  return (parseISO(iso).getDay() + 6) % 7;
}

/** Segunda-feira da semana que contém `iso`. */
export function mondayOf(iso: string): string {
  return addDays(iso, -weekdayIndex(iso));
}

/** '2026-11-23' -> '23/11' */
export function formatDM(iso: string): string {
  const [, m, d] = iso.split('-');
  return `${d}/${m}`;
}

export const WEEKDAYS = [
  'Segunda-feira',
  'Terça-feira',
  'Quarta-feira',
  'Quinta-feira',
  'Sexta-feira',
  'Sábado',
  'Domingo',
];
