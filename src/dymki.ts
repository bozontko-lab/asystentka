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
/** Dymek gratulacji z przyciskiem Cofnij trwa 10 s (B7). */
export const CZAS_DYMKU_COFNIJ_MS = 10_000;

export interface PrzyciskDymku {
  etykieta: string;
  poKliknieciu: () => void;
}

export interface OpcjeDymku {
  czasMs?: number;
  przycisk?: PrzyciskDymku;
  /** Kliknięcie w sam dymek (poza przyciskiem); dymek i tak się chowa. */
  poKliknieciu?: () => void;
}

/** Jeden dymek naraz; nowy zastępuje poprzedni. */
export class Dymek {
  private zegar: number | undefined;
  private przycisk: HTMLButtonElement | null = null;
  private poKliknieciu: (() => void) | undefined;

  constructor(
    private readonly element: HTMLElement,
    private readonly poZmianie: () => void,
  ) {
    element.addEventListener("click", () => {
      const akcja = this.poKliknieciu;
      this.schowaj();
      akcja?.();
    });
  }

  pokaz(tekst: string, opcje: OpcjeDymku = {}): void {
    window.clearTimeout(this.zegar);
    this.element.textContent = tekst;
    this.przycisk = null;
    this.poKliknieciu = opcje.poKliknieciu;
    const { przycisk } = opcje;
    if (przycisk) {
      const guzik = document.createElement("button");
      guzik.type = "button";
      guzik.className = "dymek-przycisk";
      guzik.textContent = przycisk.etykieta;
      guzik.addEventListener("click", (zdarzenie) => {
        zdarzenie.stopPropagation();
        this.schowaj();
        przycisk.poKliknieciu();
      });
      this.element.append(guzik);
      this.przycisk = guzik;
    }
    this.element.hidden = false;
    this.element.classList.remove("dymek-wejscie");
    void this.element.offsetWidth; // restart animacji wejścia
    this.element.classList.add("dymek-wejscie");
    this.zegar = window.setTimeout(() => this.schowaj(), opcje.czasMs ?? CZAS_DYMKU_MS);
    this.poZmianie();
  }

  /** Przycisk przestaje działać (minęło 10 s na Cofnij), sam dymek zostaje do końca czasu. */
  usunPrzycisk(): void {
    if (!this.przycisk) return;
    this.przycisk.remove();
    this.przycisk = null;
    this.poZmianie();
  }

  schowaj(): void {
    window.clearTimeout(this.zegar);
    this.przycisk = null;
    this.poKliknieciu = undefined;
    if (this.element.hidden) return;
    this.element.hidden = true;
    this.element.textContent = "";
    this.poZmianie();
  }
}
