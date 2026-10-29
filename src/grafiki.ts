// Grafiki postaci z części A (src/assets/postac/*.png). Brakującą grafikę
// opcjonalną zastępuje podstawowa poza z tej samej grupy (A3, B7), a dopóki
// nie ma żadnych PNG, wszystko rysuje postać zastępcza.

import { adresZastepczy } from "./zastepcza";

export const POZY = [
  "stand_idle",
  "stand_idle_blink",
  "wave_1",
  "wave_2",
  "point",
  "sit_watch",
  "sit_watch_blink",
  "sit_watch_front",
  "sit_console_1",
  "sit_console_2",
  "sit_drink",
  "pause_stretch",
  "pause_nap",
  "idle_check",
  "cheer_1",
  "cheer_2",
  "bye",
  "loading",
] as const;

export type Poza = (typeof POZY)[number];

/** Grafiki opcjonalne i poza, która je zastępuje. */
export const ZAMIENNIKI: Partial<Record<Poza, Poza>> = {
  wave_2: "wave_1",
  point: "stand_idle",
  sit_watch_front: "sit_watch",
  sit_console_2: "sit_console_1",
  sit_drink: "sit_watch",
  pause_nap: "pause_stretch",
  cheer_2: "cheer_1",
  loading: "stand_idle",
};

const PNG = import.meta.glob("./assets/postac/*.png", {
  eager: true,
  query: "?url",
  import: "default",
}) as Record<string, string>;

function plikiZPakietu(): Map<string, string> {
  const mapa = new Map<string, string>();
  for (const [sciezka, adres] of Object.entries(PNG)) {
    const nazwa = sciezka.split("/").pop()?.replace(/\.png$/i, "").toLowerCase();
    if (nazwa) mapa.set(nazwa, adres);
  }
  return mapa;
}

export class Grafiki {
  constructor(private readonly pliki: Map<string, string> = plikiZPakietu()) {}

  /** Bez żadnego PNG działa wyłącznie postać zastępcza. */
  get zastepcza(): boolean {
    return !POZY.some((p) => this.pliki.has(p));
  }

  /** Adres obrazka pozy z zastępowaniem brakujących grafik. */
  adres(poza: Poza): string {
    const plik = this.pliki.get(poza);
    if (plik) return plik;
    if (this.zastepcza) return adresZastepczy(poza);
    const zamiennik = ZAMIENNIKI[poza];
    return zamiennik ? this.adres(zamiennik) : adresZastepczy(poza);
  }

  /** Czy poza ma własną grafikę (a nie zamiennik), np. druga klatka machania. */
  ma(poza: Poza): boolean {
    return this.zastepcza || this.pliki.has(poza);
  }

  /** Wszystkie adresy, żeby przeglądarka załadowała je zawczasu. */
  wszystkie(): string[] {
    return [...new Set(POZY.map((p) => this.adres(p)))];
  }
}
