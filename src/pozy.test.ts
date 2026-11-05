import { describe, expect, it } from "vitest";

import type { Poza } from "./grafiki";
import { type DostepneGrafiki, klatkiPostaci, type StanPostaci } from "./pozy";
import type { KoniecWidok } from "./typy";

const SEK = 1000;
const MIN = 60 * SEK;

/** Komplet grafik: każda poza ma własny plik. */
const wszystkie: DostepneGrafiki = {
  ma: () => true,
  adres: (p: Poza) => `${p}.png`,
  reakcje: { zachwycona: ["cheer_1", "cheer_2"], zmartwiona: ["concerned"], czula: ["stand_idle"] },
};

/** Tylko podstawowe pozy: opcjonalne zastępuje podstawowa z tej samej grupy (A3). */
const podstawowe: DostepneGrafiki = {
  ma: (p: Poza) => !["wave_2", "cheer_2", "sit_watch_front", "sit_drink", "point"].includes(p),
  adres: (p: Poza) => `${({ wave_2: "wave_1", cheer_2: "cheer_1" } as Record<string, string>)[p] ?? p}.png`,
  reakcje: {},
};

function stan(zmiany: Partial<StanPostaci>): StanPostaci {
  return {
    stan: "lista",
    krokKreatora: 0,
    wskazuje: false,
    przeciaga: false,
    blisko: false,
    wariant: "patrzy",
    czasPrzerwyMs: 0,
    konsolkaDwa: false,
    koniec: null,
    ...zmiany,
  };
}

const koniec = (zmiany: Partial<KoniecWidok>): KoniecWidok =>
  ({ etap: "reakcja", reakcja: "zachwycona", podsumowanie: {}, ...zmiany }) as KoniecWidok;

describe("pozy w stanach z B4", () => {
  it("kreator macha na pierwszym kroku, potem wskazuje plik", () => {
    expect(klatkiPostaci(stan({ stan: "konfiguracja" }), wszystkie)).toEqual({ a: "wave_1", b: "wave_2", tryb: "klatki", cykl: "0.7s" });
    expect(klatkiPostaci(stan({ stan: "konfiguracja", krokKreatora: 2 }), wszystkie).a).toBe("point");
  });

  it("powitanie macha dwiema klatkami, a bez drugiej klatki stoi na jednej", () => {
    expect(klatkiPostaci(stan({ stan: "powitanie" }), wszystkie).tryb).toBe("klatki");
    expect(klatkiPostaci(stan({ stan: "powitanie" }), podstawowe)).toEqual({ a: "wave_1", tryb: "brak" });
  });

  it("wybór zadań: najpierw wskazuje, potem stoi z mrugnięciem; lista stoi", () => {
    expect(klatkiPostaci(stan({ stan: "wybor_zadan", wskazuje: true }), wszystkie).a).toBe("point");
    expect(klatkiPostaci(stan({ stan: "wybor_zadan" }), wszystkie)).toEqual({ a: "stand_idle", b: "stand_idle_blink", tryb: "mrug" });
    expect(klatkiPostaci(stan({ stan: "lista" }), wszystkie).a).toBe("stand_idle");
  });

  it("praca: patrzy z mrugnięciem, konsolka, kubek, przeciąganie", () => {
    expect(klatkiPostaci(stan({ stan: "praca" }), wszystkie)).toEqual({ a: "sit_watch", b: "sit_watch_blink", tryb: "mrug" });
    expect(klatkiPostaci(stan({ stan: "praca", wariant: "konsola" }), wszystkie).a).toBe("sit_console_1");
    expect(klatkiPostaci(stan({ stan: "praca", wariant: "konsola", konsolkaDwa: true }), wszystkie).a).toBe("sit_console_2");
    expect(klatkiPostaci(stan({ stan: "praca", wariant: "kubek" }), wszystkie).a).toBe("sit_drink");
    expect(klatkiPostaci(stan({ stan: "praca", przeciaga: true, wariant: "kubek" }), wszystkie).a).toBe("pause_stretch");
  });

  it("praca: kursor blisko przerywa tylko patrzenie (decyzja właściciela)", () => {
    expect(klatkiPostaci(stan({ stan: "praca", blisko: true }), wszystkie).a).toBe("sit_watch_front");
    expect(klatkiPostaci(stan({ stan: "praca", blisko: true, wariant: "konsola" }), wszystkie).a).toBe("sit_console_1");
    expect(klatkiPostaci(stan({ stan: "praca", blisko: true, wariant: "kubek" }), wszystkie).a).toBe("sit_drink");
    expect(klatkiPostaci(stan({ stan: "praca", blisko: true }), podstawowe).a, "bez sit_watch_front zostaje sit_watch").toBe("sit_watch");
  });

  it("przerwa: 5 s przeciągania, potem konsolka, od 15 min drzemka", () => {
    expect(klatkiPostaci(stan({ stan: "pauza", czasPrzerwyMs: 0 }), wszystkie)).toEqual({ a: "pause_stretch", tryb: "brak", mocne: true });
    expect(klatkiPostaci(stan({ stan: "pauza", czasPrzerwyMs: 6 * SEK }), wszystkie).a).toBe("sit_console_1");
    expect(klatkiPostaci(stan({ stan: "pauza", czasPrzerwyMs: 6 * MIN, konsolkaDwa: true }), wszystkie).a).toBe("sit_console_2");
    expect(klatkiPostaci(stan({ stan: "pauza", czasPrzerwyMs: 20 * MIN }), wszystkie).a).toBe("pause_nap");
  });

  it("„Jesteś?” i gratulacje", () => {
    expect(klatkiPostaci(stan({ stan: "jestes" }), wszystkie).a).toBe("idle_check");
    expect(klatkiPostaci(stan({ stan: "gratulacje" }), wszystkie)).toEqual({ a: "cheer_1", b: "cheer_2", tryb: "klatki", cykl: "0.8s" });
    expect(klatkiPostaci(stan({ stan: "gratulacje" }), podstawowe)).toEqual({ a: "cheer_1", tryb: "brak" });
  });

  it("koniec dnia: reakcja z tabeli, pożegnanie, sen", () => {
    expect(klatkiPostaci(stan({ stan: "koniec_dnia", koniec: koniec({}) }), wszystkie).tryb).toBe("klatki");
    expect(klatkiPostaci(stan({ stan: "koniec_dnia", koniec: koniec({ reakcja: "zmartwiona" }) }), wszystkie).a).toBe("concerned");
    expect(klatkiPostaci(stan({ stan: "koniec_dnia", koniec: koniec({ reakcja: "czula" }) }), wszystkie).tryb).toBe("mrug");
    expect(klatkiPostaci(stan({ stan: "koniec_dnia", koniec: koniec({ etap: "pozegnanie" }) }), wszystkie).a).toBe("bye");
    expect(klatkiPostaci(stan({ stan: "koniec_dnia", koniec: koniec({ etap: "spi" }) }), wszystkie).a).toBe("pause_nap");
    expect(klatkiPostaci(stan({ stan: "koniec_dnia" }), wszystkie).a, "bez podsumowania stoi").toBe("stand_idle");
  });

  it("przed pierwszą migawką stoi", () => {
    expect(klatkiPostaci(stan({ stan: null }), wszystkie).a).toBe("stand_idle");
  });
});
