// Dymki z src/kwestie.json (SPEC.md, B7): losowy wariant dla zdarzenia,
// podstawione wstawki, znika po 6 s albo po kliknięciu, naraz tylko jeden.

import kwestieDomyslne from "./kwestie.json";

export type Kwestie = Record<string, string[]>;

const WSTAWKA = /\{(\w+)\}/g;

function wstawkiWzoru(wzor: string): string[] {
  return [...wzor.matchAll(WSTAWKA)].map((m) => m[1] ?? "");
}

/**
 * Wybiera wariant, dla którego znamy wszystkie wstawki. Gdy żaden nie
 * pasuje (np. brak {dni} przed nadejściem planu), zwraca `null`.
 */
export function kwestia(
  klucz: string,
  wstawki: Record<string, string>,
  kwestie: Kwestie = kwestieDomyslne,
  losuj: () => number = Math.random,
): string | null {
  const pasujace = (kwestie[klucz] ?? []).filter((wzor) =>
    wstawkiWzoru(wzor).every((nazwa) => (wstawki[nazwa] ?? "") !== ""),
  );
  if (pasujace.length === 0) return null;
  const wzor = pasujace[Math.min(pasujace.length - 1, Math.floor(losuj() * pasujace.length))] ?? "";
  return wzor.replace(WSTAWKA, (_, nazwa: string) => wstawki[nazwa] ?? "");
}

export const CZAS_DYMKU_MS = 6000;

/** Jeden dymek naraz; nowy zastępuje poprzedni. */
export class Dymek {
  private zegar: number | undefined;

  constructor(
    private readonly element: HTMLElement,
    private readonly poZmianie: () => void,
  ) {
    element.addEventListener("click", () => this.schowaj());
  }

  pokaz(tekst: string, czasMs = CZAS_DYMKU_MS): void {
    window.clearTimeout(this.zegar);
    this.element.textContent = tekst;
    this.element.hidden = false;
    this.element.classList.remove("dymek-wejscie");
    void this.element.offsetWidth; // restart animacji wejścia
    this.element.classList.add("dymek-wejscie");
    this.zegar = window.setTimeout(() => this.schowaj(), czasMs);
    this.poZmianie();
  }

  schowaj(): void {
    window.clearTimeout(this.zegar);
    if (this.element.hidden) return;
    this.element.hidden = true;
    this.element.textContent = "";
    this.poZmianie();
  }
}
