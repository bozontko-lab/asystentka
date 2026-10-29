// Formatowanie czasu po polsku, spójne z dziennikiem (src-tauri/src/czas.rs).

const dwie = (n: number) => String(n).padStart(2, "0");

/** Licznik: mm:ss, a od godziny h:mm:ss (SPEC.md, B5). */
export function licznik(sekundy: number): string {
  const s = Math.max(0, Math.floor(sekundy));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const r = s % 60;
  return h > 0 ? `${h}:${dwie(m)}:${dwie(r)}` : `${dwie(m)}:${dwie(r)}`;
}

/** `47 min`, `2 h 12 min`, `1 h` — jak w dzienniku. */
export function minutyTekst(minuty: number): string {
  const h = Math.floor(minuty / 60);
  const m = minuty % 60;
  if (h === 0) return `${m} min`;
  if (m === 0) return `${h} h`;
  return `${h} h ${m} min`;
}

/** Zaokrąglenie do pełnych minut (od 30 s w górę), jak w dzienniku. */
export function naMinuty(sekundy: number): number {
  return sekundy <= 0 ? 0 : Math.floor((sekundy + 30) / 60);
}

/** Szacunek obok licznika: `47 / 60 min`, a bez szacunku `47 min`. */
export function szacunekTekst(minuty: number, szac: number | null): string {
  return szac == null ? `${minuty} min` : `${minuty} / ${szac} min`;
}

/** `07:40` → `7:40`. */
export function godzinaBezZera(tekst: string): string {
  return tekst.replace(/^0(\d):/, "$1:");
}

/** `2026-11-05` → `5.11`. */
export function terminTekst(iso: string): string {
  const m = /^\d{4}-(\d{2})-(\d{2})$/.exec(iso);
  return m ? `${Number(m[2])}.${Number(m[1])}` : iso;
}
