import { describe, expect, it } from "vitest";

import { czasWiersza, czescSzacunku, moznaZaczac, minutyBezczynnosci, ponadSzacunkiem, sumaWyboru } from "./widok";

describe("wiersz listy planu", () => {
  it("pokazuje ✓, czas względem szacunku albo sam szacunek", () => {
    expect(czasWiersza({ zrobione: true, czas_min: 47, szac_min: 60 })).toBe("✓");
    expect(czasWiersza({ zrobione: false, czas_min: 45, szac_min: 120 })).toBe("45 / 120 min");
    expect(czasWiersza({ zrobione: false, czas_min: 0, szac_min: 30 })).toBe("30 min");
    expect(czasWiersza({ zrobione: false, czas_min: 0, szac_min: null })).toBe("");
  });

  it("po przekroczeniu szacunku czas jest koralowy, ale nie przy zrobionym ani bez szacunku", () => {
    expect(ponadSzacunkiem({ zrobione: false, czas_min: 61, szac_min: 60 })).toBe(true);
    expect(ponadSzacunkiem({ zrobione: false, czas_min: 60, szac_min: 60 })).toBe(false);
    expect(ponadSzacunkiem({ zrobione: true, czas_min: 90, szac_min: 60 })).toBe(false);
    expect(ponadSzacunkiem({ zrobione: false, czas_min: 90, szac_min: null })).toBe(false);
  });
});

describe("wybór zadań", () => {
  it("„Zaczynamy” działa z zaznaczonym zadaniem albo przy powrocie do planu", () => {
    expect(moznaZaczac({ zaznaczonych: 0, mozna_wrocic: false })).toBe(false);
    expect(moznaZaczac({ zaznaczonych: 2, mozna_wrocic: false })).toBe(true);
    expect(moznaZaczac({ zaznaczonych: 0, mozna_wrocic: true })).toBe(true);
  });

  it("suma pokazuje czas zadań i cel", () => {
    expect(sumaWyboru({ suma_min: 75, cel_min: 240 })).toBe("Razem 1 h 15 min z celu 4 h");
  });
});

describe("panel Pracy", () => {
  it("pasek czasu to część szacunku, powyżej 1 po przekroczeniu", () => {
    expect(czescSzacunku(30 * 60, 60)).toBe(0.5);
    expect(czescSzacunku(90 * 60, 60)).toBe(1.5);
    expect(czescSzacunku(600, null), "bez szacunku pasek stoi").toBe(0);
  });

  it("„Przerwa (odejmij X min)” liczy pełne minuty bez ruchu, co najmniej 1", () => {
    expect(minutyBezczynnosci(0, 16 * 60_000)).toBe(16);
    expect(minutyBezczynnosci(0, 10_000)).toBe(1);
  });
});
