// Wybór pozy postaci dla stanu (SPEC.md, B4, B7), bez DOM i bez zegarów, żeby dało się
// go sprawdzić testami. `Postac` zbiera stan i rysuje to, co tu wyjdzie.

import type { Grafiki, Poza } from "./grafiki";
import { fazaPrzerwy } from "./przerwa";
import type { KoniecWidok, NazwaStanu } from "./typy";

export type Tryb = "brak" | "mrug" | "klatki";

export interface Klatki {
  a: Poza;
  b?: Poza;
  tryb: Tryb;
  /** Długość cyklu dwóch klatek, np. machania. */
  cykl?: string;
  /** Początek przerwy: jedno mocniejsze przeciągnięcie (skala w pionie do 1,03 i z powrotem). */
  mocne?: boolean;
}

/** Pozy siedzące w Pracy, losowane co 3–6 min. */
export type Wariant = "patrzy" | "konsola" | "kubek";

/** Wszystko, od czego zależy poza w danej chwili. */
export interface StanPostaci {
  stan: NazwaStanu | null;
  /** Krok kreatora: przy pierwszym postać macha, potem wskazuje plik. */
  krokKreatora: number;
  /** Wybór zadań: przez 3 s wskazuje listę. */
  wskazuje: boolean;
  /** Przypomnienie o przerwie: chwilę się przeciąga. */
  przeciaga: boolean;
  /** Kursor bliżej niż 300 px. */
  blisko: boolean;
  wariant: Wariant;
  /** Czas od początku przerwy (ms). */
  czasPrzerwyMs: number;
  /** Na konsolce widać właśnie `sit_console_2`. */
  konsolkaDwa: boolean;
  koniec: KoniecWidok | null;
}

/** Tyle z `Grafiki`, ile potrzeba do wyboru pozy. */
export type DostepneGrafiki = Pick<Grafiki, "ma" | "adres" | "reakcje">;

const STOI: Klatki = { a: "stand_idle", b: "stand_idle_blink", tryb: "mrug" };

function dwie(g: DostepneGrafiki, a: Poza, b: Poza, cykl: string): Klatki {
  return g.ma(b) && g.adres(a) !== g.adres(b) ? { a, b, tryb: "klatki", cykl } : { a, tryb: "brak" };
}

function konsolka(s: StanPostaci): Klatki {
  return { a: s.konsolkaDwa ? "sit_console_2" : "sit_console_1", tryb: "brak" };
}

/** Koniec dnia: reakcja z tabeli w B4 (pozy z postac.json), potem „bye”, a po schowaniu „pause_nap”. */
function klatkiKonca(k: KoniecWidok | null, g: DostepneGrafiki): Klatki {
  if (!k) return STOI;
  if (k.etap === "pozegnanie") return { a: "bye", tryb: "brak" };
  if (k.etap === "spi") return { a: "pause_nap", tryb: "brak" };
  const [a, b] = (g.reakcje[k.reakcja] ?? ["stand_idle"]) as Poza[];
  if (a && b) return dwie(g, a, b, "0.8s");
  return a === "stand_idle" ? STOI : { a: a ?? "stand_idle", tryb: "brak" };
}

/** Poza i tryb animacji dla stanu postaci. */
export function klatkiPostaci(s: StanPostaci, g: DostepneGrafiki): Klatki {
  switch (s.stan) {
    case "konfiguracja":
      return s.krokKreatora === 0 ? dwie(g, "wave_1", "wave_2", "0.7s") : { a: "point", tryb: "brak" };
    case "powitanie":
      return dwie(g, "wave_1", "wave_2", "0.7s");
    case "wybor_zadan":
      return s.wskazuje ? { a: "point", tryb: "brak" } : STOI;
    case "lista":
      return STOI;
    case "praca":
      if (s.przeciaga) return { a: "pause_stretch", tryb: "brak" };
      // Konsolki ani kubka kursor nie przerywa; spojrzenie dotyczy tylko patrzenia.
      if (s.wariant === "konsola") return konsolka(s);
      if (s.wariant === "kubek") return { a: "sit_drink", tryb: "brak" };
      if (s.blisko && g.ma("sit_watch_front")) return { a: "sit_watch_front", tryb: "brak" };
      return { a: "sit_watch", b: "sit_watch_blink", tryb: "mrug" };
    case "pauza":
      // 0–5 s przeciąganie, do 15 min konsolka, potem drzemka (B4, B7).
      switch (fazaPrzerwy(s.czasPrzerwyMs)) {
        case "rozciaganie":
          return { a: "pause_stretch", tryb: "brak", mocne: true };
        case "konsola":
          return konsolka(s);
        case "drzemka":
          return { a: "pause_nap", tryb: "brak" };
      }
      return STOI;
    case "jestes":
      return { a: "idle_check", tryb: "brak" };
    case "gratulacje":
      return dwie(g, "cheer_1", "cheer_2", "0.8s");
    case "koniec_dnia":
      return klatkiKonca(s.koniec, g);
    default:
      return STOI;
  }
}
