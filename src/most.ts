// Most do backendu: komendy i zdarzenia Tauri. Podgląd w przeglądarce
// (npm run dev bez Tauri) podstawia własną implementację z podglad.ts.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { error as logBlad } from "@tauri-apps/plugin-log";

import type { Migawka, PodpowiedzKreatora, Ustawienia, Zdarzenie } from "./typy";

export type Akcja =
  | "pauza"
  | "wznow"
  | "zrobione"
  | "przerwij"
  | "wroc_do_listy"
  | "zatwierdz_wybor"
  | "anuluj_wybor"
  | "zmien_plan"
  | "koniec_pracy"
  | "do_jutra"
  | "wroc_do_pracy"
  | "dalej_po_wczoraju";

export interface Most {
  migawka(): Promise<Migawka>;
  gotowy(szerokosc: number, wysokosc: number): Promise<void>;
  dopasuj(szerokosc: number, wysokosc: number): Promise<void>;
  rozpocznij(projekt: string, zadanie: string): Promise<void>;
  przelaczWybor(projekt: string, zadanie: string): Promise<void>;
  /** Dopisuje zadanie do pliku i do planu (w Wyborze: do zaznaczonych). */
  dodajZadanie(nazwa: string, szacMin: number | null, projekt: string): Promise<void>;
  akcja(akcja: Akcja): Promise<void>;
  odpowiedzJestes(przerwa: boolean): Promise<void>;
  naMigawke(obsluga: (m: Migawka) => void): Promise<unknown>;
  naZdarzenie(obsluga: (z: Zdarzenie) => void): Promise<unknown>;
  naKursor(obsluga: (blisko: boolean) => void): Promise<unknown>;
  /** Polecenia z menu pod prawym przyciskiem, które wymagają widoku (np. „dodaj_zadanie”). */
  naAkcja(obsluga: (nazwa: string) => void): Promise<unknown>;
  /** Prawy przycisk na postaci albo panelu: menu z poleceniami. */
  menuKontekstowe(): Promise<void>;
  przeciagnij(): void;
  /** Błąd frontendu do pliku logu (na Windows konsoli WebView2 nie widać). */
  zglosBlad(tekst: string): void;
  kreatorDane(): Promise<PodpowiedzKreatora>;
  kreatorZakoncz(ustawienia: Ustawienia, utworzPlik: boolean): Promise<void>;
  plikIstnieje(sciezka: string): Promise<boolean>;
  wybierzPlik(start?: string): Promise<string | null>;
  wybierzFolder(start?: string, tytul?: string): Promise<string | null>;
  /** Okno widgetu nie przejmuje fokusu; pola tekstowe włączają go na czas wpisywania. */
  fokus(wlacz: boolean): Promise<void>;
  ustawieniaPobierz(): Promise<Ustawienia>;
  ustawieniaZapisz(ustawienia: Ustawienia): Promise<void>;
  otworzUstawienia(): Promise<void>;
  zamknijOkno(): Promise<void>;
}

export function wTauri(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

export function mostTauri(): Most {
  return {
    migawka: () => invoke<Migawka>("migawka"),
    gotowy: (szerokosc, wysokosc) => invoke("gotowy", { szerokosc, wysokosc }),
    dopasuj: (szerokosc, wysokosc) => invoke("dopasuj_okno", { szerokosc, wysokosc }),
    rozpocznij: (projekt, zadanie) => invoke("rozpocznij", { projekt, zadanie }),
    przelaczWybor: (projekt, zadanie) => invoke("przelacz_wybor", { projekt, zadanie }),
    dodajZadanie: (nazwa, szacMin, projekt) => invoke("dodaj_zadanie", { nazwa, szacMin, projekt }),
    akcja: (akcja) => invoke(akcja),
    odpowiedzJestes: (przerwa) => invoke("odpowiedz_jestes", { przerwa }),
    naMigawke: (obsluga) => listen<Migawka>("migawka", (e) => obsluga(e.payload)),
    naZdarzenie: (obsluga) => listen<Zdarzenie>("zdarzenie", (e) => obsluga(e.payload)),
    naKursor: (obsluga) => listen<boolean>("kursor", (e) => obsluga(e.payload)),
    naAkcja: (obsluga) => listen<string>("akcja", (e) => obsluga(e.payload)),
    menuKontekstowe: () => invoke("menu_kontekstowe"),
    przeciagnij: () => {
      void getCurrentWindow().startDragging();
    },
    kreatorDane: () => invoke<PodpowiedzKreatora>("kreator_dane"),
    kreatorZakoncz: (ustawienia, utworzPlik) => invoke("kreator_zakoncz", { ustawienia, utworzPlik }),
    plikIstnieje: (sciezka) => invoke<boolean>("plik_istnieje", { sciezka }),
    wybierzPlik: (start) => invoke<string | null>("wybierz_plik", { start }),
    wybierzFolder: (start, tytul) => invoke<string | null>("wybierz_folder", { start, tytul }),
    fokus: (wlacz) => invoke("ustaw_fokus", { wlacz }),
    ustawieniaPobierz: () => invoke<Ustawienia>("ustawienia_pobierz"),
    ustawieniaZapisz: (ustawienia) => invoke("ustawienia_zapisz", { ustawienia }),
    otworzUstawienia: () => invoke("otworz_ustawienia"),
    zamknijOkno: () => getCurrentWindow().close(),
    zglosBlad: (tekst) => {
      console.error(tekst);
      void logBlad(`frontend: ${tekst}`).catch(() => undefined);
    },
  };
}
