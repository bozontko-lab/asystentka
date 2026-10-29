// Most do backendu: komendy i zdarzenia Tauri. Podgląd w przeglądarce
// (npm run dev bez Tauri) podstawia własną implementację z podglad.ts.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { error as logBlad } from "@tauri-apps/plugin-log";

import type { Migawka, Zdarzenie } from "./typy";

export type Akcja = "pauza" | "wznow" | "zrobione" | "przerwij" | "pokaz_liste";

export interface Most {
  migawka(): Promise<Migawka>;
  gotowy(szerokosc: number, wysokosc: number): Promise<void>;
  dopasuj(szerokosc: number, wysokosc: number): Promise<void>;
  rozpocznij(id: string): Promise<void>;
  akcja(akcja: Akcja): Promise<void>;
  odpowiedzJestes(przerwa: boolean): Promise<void>;
  naMigawke(obsluga: (m: Migawka) => void): Promise<unknown>;
  naZdarzenie(obsluga: (z: Zdarzenie) => void): Promise<unknown>;
  naKursor(obsluga: (blisko: boolean) => void): Promise<unknown>;
  przeciagnij(): void;
  /** Błąd frontendu do pliku logu (na Windows konsoli WebView2 nie widać). */
  zglosBlad(tekst: string): void;
}

export function wTauri(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

export function mostTauri(): Most {
  return {
    migawka: () => invoke<Migawka>("migawka"),
    gotowy: (szerokosc, wysokosc) => invoke("gotowy", { szerokosc, wysokosc }),
    dopasuj: (szerokosc, wysokosc) => invoke("dopasuj_okno", { szerokosc, wysokosc }),
    rozpocznij: (id) => invoke("rozpocznij", { id }),
    akcja: (akcja) => invoke(akcja),
    odpowiedzJestes: (przerwa) => invoke("odpowiedz_jestes", { przerwa }),
    naMigawke: (obsluga) => listen<Migawka>("migawka", (e) => obsluga(e.payload)),
    naZdarzenie: (obsluga) => listen<Zdarzenie>("zdarzenie", (e) => obsluga(e.payload)),
    naKursor: (obsluga) => listen<boolean>("kursor", (e) => obsluga(e.payload)),
    przeciagnij: () => {
      void getCurrentWindow().startDragging();
    },
    zglosBlad: (tekst) => {
      console.error(tekst);
      void logBlad(`frontend: ${tekst}`).catch(() => undefined);
    },
  };
}
