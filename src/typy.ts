// Typy danych z backendu (src-tauri/src/rdzen.rs). Nazwy pól jak w Ruście.

export type NazwaStanu =
  | "konfiguracja"
  | "powitanie"
  | "wybor_zadan"
  | "lista"
  | "praca"
  | "pauza"
  | "jestes"
  | "gratulacje"
  | "koniec_dnia";

export type Strona = "prawa" | "lewa";
/** Przy której krawędzi monitora stoi widget; od niej panel rośnie w stronę środka. */
export type Pion = "dol" | "gora";

export type PowodPauzy = "reczna" | "blokada" | "uspienie" | "zamkniecie" | "awaria";

/** Zadanie rozpoznaje para: projekt i tekst linii bez pól (B3). */
export interface KluczZadania {
  projekt: string;
  zadanie: string;
}

export interface ZadanieWidok extends KluczZadania {
  szac_min: number | null;
  /** Łączny czas z pliku. */
  czas_min: number;
  zrobione: boolean;
  w_toku: boolean;
  zaznaczone: boolean;
  termin: string | null;
}

export interface ProjektWidok {
  nazwa: string;
  zadania: ZadanieWidok[];
}

export interface WyborWidok {
  projekty: ProjektWidok[];
  suma_min: number;
  cel_min: number;
  przekroczony: boolean;
  zaznaczonych: number;
  mozna_wrocic: boolean;
}

export interface ListaWidok {
  zadania: ZadanieWidok[];
  zrobione: number;
  razem: number;
  pozostalo_min: number;
}

export interface SesjaWidok extends KluczZadania {
  szac_min: number | null;
  /** Czas z pliku z chwili startu plus zamknięte odcinki; otwarty odcinek dolicza frontend. */
  sekundy_zamkniete: number;
  odcinek_od_ms: number | null;
  pauza_od_ms: number | null;
  powod_pauzy: PowodPauzy | null;
  bezczynna_od_ms: number | null;
}

export interface Gratulacje {
  zadanie: string;
  czas_min: number;
  szac_min: number | null;
  czas_dnia_min: number;
  plan_wykonany: boolean;
}

export type Reakcja = "zachwycona" | "zadowolona" | "dumna" | "lekko_pochwalajaca" | "czula" | "zmartwiona";

export interface ZadanieDniaWidok {
  zadanie: string;
  projekt: string;
  czas_min: number;
  szac_min: number | null;
  zrobione: boolean;
}

export interface PodsumowanieWidok {
  zrobione: number;
  zaplanowane: number;
  czas_dnia_min: number;
  cel_min: number;
  zadania: ZadanieDniaWidok[];
  na_jutro: string[];
}

export interface KoniecWidok {
  etap: "reakcja" | "pozegnanie" | "spi" | "wczoraj";
  reakcja: Reakcja;
  podsumowanie: PodsumowanieWidok;
}

export interface Migawka {
  stan: NazwaStanu;
  widoczna: boolean;
  /** Lewa połowa monitora widgetu: postać odbija się lustrzanie. */
  strona: Strona;
  pion: Pion;
  imie: string;
  wybor: WyborWidok | null;
  lista: ListaWidok | null;
  sesja: SesjaWidok | null;
  gratulacje: Gratulacje | null;
  koniec: KoniecWidok | null;
  dzien: { minuty: number; cel_min: number };
  blad_pliku: string | null;
  /** Projekty do wyboru w „Dodaj zadanie”: nagłówki z pliku i Inbox. */
  projekty_do_dopisania: string[];
}

export type Zdarzenie =
  | { rodzaj: "dymek"; klucz: string; wstawki: Record<string, string> }
  | { rodzaj: "efekt"; nazwa: string };

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
