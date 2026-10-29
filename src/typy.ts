// Typy danych z backendu (src-tauri/src/rdzen.rs). Nazwy pól jak w Ruście.

export type NazwaStanu =
  | "powitanie"
  | "lista"
  | "konfiguracja"
  | "czeka_na_plan"
  | "praca"
  | "pauza"
  | "jestes"
  | "gratulacje"
  | "koniec_dnia";

export type Strona = "prawa" | "lewa";

export type PowodPauzy = "reczna" | "blokada" | "uspienie" | "zamkniecie" | "awaria";

export interface ZadanieWidok {
  id: string;
  nazwa: string;
  etap: string | null;
  szac_min: number | null;
  status: string | null;
  minuty_dzis: number;
  minuty_razem: number;
}

export interface PlanWidok {
  naglowek: string;
  zagrozone: boolean;
  dni_do_publikacji: number | null;
  zadania: ZadanieWidok[];
  zapasowe: ZadanieWidok[];
}

export interface SesjaWidok {
  id: string;
  nazwa: string;
  szac_min: number | null;
  sekundy_zamkniete: number;
  odcinek_od_ms: number | null;
  pauza_od_ms: number | null;
  powod_pauzy: PowodPauzy | null;
  bezczynna_od_ms: number | null;
  minuty_wczesniej: number;
}

export interface Gratulacje {
  nazwa: string;
  minuty: number;
  szac_min: number | null;
  ostatnie: boolean;
}

export interface Migawka {
  stan: NazwaStanu;
  widoczna: boolean;
  strona: Strona;
  imie: string;
  plan: PlanWidok | null;
  w_toku: ZadanieWidok[];
  sesja: SesjaWidok | null;
  gratulacje: Gratulacje | null;
  dzien: { minuty: number; zrobione: string[] };
  godzina_planu: string;
  brakujace_sciezki: string[];
}

export type Zdarzenie =
  | { rodzaj: "dymek"; klucz: string; wstawki: Record<string, string> }
  | { rodzaj: "efekt"; nazwa: string };

export const ZROBIONE = "Zrobione";
export const W_TOKU = "W toku";

/** Ustawienia z %APPDATA%\Asystentka\ustawienia.json (SPEC.md, B7). */
export interface Ustawienia {
  imie: string;
  plik_zadan: string;
  folder_dziennika: string;
  cel_dzienny_min: number;
  godzina_konca_dnia: string;
  autostart: boolean;
  prog_bezczynnosci_min: number;
  przypomnienie_przerwy_min: number;
  ukryj_przed_nagrywaniem: boolean;
  chowaj_przy_pelnym_ekranie: boolean;
  skrot_pokaz_ukryj: string;
}

export interface PodpowiedzKreatora {
  ustawienia: Ustawienia;
  /** Wskazany plik zadań już istnieje, więc kreator ma go użyć. */
  plik_istnieje: boolean;
  vaulty: string[];
}
