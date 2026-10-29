// Grafiki postaci z części A (src/assets/postac/*.png). Brakującą grafikę
// opcjonalną zastępuje podstawowa poza z tej samej grupy (A3, B7), a dopóki
// nie ma żadnych PNG, wszystko rysuje postać zastępcza. Rozmiar postaci i linie
// stóp oraz siedziska opisuje src/assets/postac/postac.json.

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
  "concerned",
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
  concerned: "stand_idle",
};

/** Opis grafik z postac.json: płótno, rozmiar na ekranie i linie póz. */
export interface Manifest {
  /** [szerokość, wysokość] płótna w pikselach obrazka. */
  plotno_px?: number[];
  /** [szerokość, wysokość] postaci na ekranie w pikselach CSS. */
  wyswietlanie_css_px?: number[];
  pozy?: Record<string, { siedzisko_y_px?: number; stopy_y_px?: number }>;
}

/** Wymiary postaci na ekranie w pikselach CSS. */
export interface Geometria {
  szer: number;
  wys: number;
  /** Część obrazka poniżej linii siedziska: w Pracy i Pauzie chowa się za panelem. */
  podSiedziskiem: number;
  /** Margines pod stopami: o tyle stojąca postać zachodzi na panel. */
  podStopami: number;
  /** Górna skala oddechu: 1,015 dla chibi, 1,008 dla wysokiej postaci (A2b). */
  oddech: number;
}

/** Postać zastępcza: kwadrat 200 px jak w wartościach startowych z B6. */
export const GEOMETRIA_ZASTEPCZA: Geometria = { szer: 200, wys: 200, podSiedziskiem: 8, podStopami: 4, oddech: 1.015 };

/** Od tej wysokości postać traktujemy jak wariant B z A2b (280 px, delikatniejszy oddech). */
const WYSOKA_PX = 260;

function mediana(liczby: number[]): number | undefined {
  if (liczby.length === 0) return undefined;
  const posortowane = [...liczby].sort((a, b) => a - b);
  const srodek = Math.floor(posortowane.length / 2);
  return posortowane.length % 2 ? posortowane[srodek] : ((posortowane[srodek - 1] ?? 0) + (posortowane[srodek] ?? 0)) / 2;
}

/**
 * Geometria z manifestu. Wszystkie pozy leżą na tym samym płótnie, więc dla
 * całej grupy wystarczy jedna linia: siedzisko podstawowej pozy siedzącej
 * (`sit_watch`) i stopy podstawowej pozy stojącej (`stand_idle`).
 */
export function geometriaZManifestu(manifest: Manifest | undefined): Geometria {
  const [, plotnoWys] = manifest?.plotno_px ?? [];
  const [szer, wys] = manifest?.wyswietlanie_css_px ?? [];
  if (!manifest || !plotnoWys || !szer || !wys) return { ...GEOMETRIA_ZASTEPCZA };
  const skala = wys / plotnoWys;
  const pozy = manifest.pozy ?? {};
  const wszystkie = (pole: "siedzisko_y_px" | "stopy_y_px") =>
    Object.values(pozy).flatMap((p) => (typeof p[pole] === "number" ? [p[pole]] : []));
  const siedzisko = pozy.sit_watch?.siedzisko_y_px ?? mediana(wszystkie("siedzisko_y_px")) ?? plotnoWys;
  const stopy = pozy.stand_idle?.stopy_y_px ?? mediana(wszystkie("stopy_y_px")) ?? plotnoWys;
  return {
    szer,
    wys,
    podSiedziskiem: Math.round((plotnoWys - siedzisko) * skala),
    podStopami: Math.round((plotnoWys - stopy) * skala),
    oddech: wys >= WYSOKA_PX ? 1.008 : 1.015,
  };
}

// Ikona i wzorzec do promptów nie są wyświetlane, więc nie trafiają do paczki.
const PNG = import.meta.glob(
  ["./assets/postac/*.png", "!./assets/postac/icon.png", "!./assets/postac/ref_master.png"],
  { eager: true, query: "?url", import: "default" },
) as Record<string, string>;

const MANIFEST = import.meta.glob("./assets/postac/postac.json", { eager: true, import: "default" }) as Record<
  string,
  Manifest
>;

function plikiZPakietu(): Map<string, string> {
  const mapa = new Map<string, string>();
  for (const [sciezka, adres] of Object.entries(PNG)) {
    const nazwa = sciezka.split("/").pop()?.replace(/\.png$/i, "").toLowerCase();
    if (nazwa) mapa.set(nazwa, adres);
  }
  return mapa;
}

export class Grafiki {
  readonly geometria: Geometria;

  constructor(
    private readonly pliki: Map<string, string> = plikiZPakietu(),
    manifest: Manifest | undefined = Object.values(MANIFEST)[0],
  ) {
    this.geometria = this.zastepcza ? { ...GEOMETRIA_ZASTEPCZA } : geometriaZManifestu(manifest);
  }

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
