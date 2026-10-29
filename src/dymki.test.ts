import { readFileSync } from "node:fs";

import { describe, expect, it } from "vitest";

import { kwestia } from "./dymki";
import kwestie from "./kwestie.json";

describe("kwestia", () => {
  it("podstawia wstawki z rzeczownikiem w poprawnej formie", () => {
    const pierwszy = () => 0;
    expect(kwestia("powitanie", { imie: "Dżiordżio", dni: "6 dni" }, kwestie, pierwszy)).toBe(
      "Dzień dobry, Dżiordżio! Do premiery 6 dni.",
    );
    expect(kwestia("czeka_na_plan", { godzina_planu: "7:40" }, kwestie, pierwszy)).toBe(
      "Plan jeszcze się układa, będzie po 7:40.",
    );
  });

  it("pomija warianty z nieznanymi wstawkami", () => {
    // Bez {dni} zostaje tylko drugi wariant powitania.
    expect(kwestia("powitanie", { imie: "Ola" }, kwestie, () => 0)).toBe("Hej! Plan na dziś gotowy, rzuć okiem.");
    expect(kwestia("przypomnienie_przerwy", {}, kwestie)).toBeNull();
    expect(kwestia("nie_ma_takiego", {}, kwestie)).toBeNull();
  });

  it("losuje spośród pasujących wariantów", () => {
    const wstawki = { zadanie: "Montaż", minuty: "47 min" };
    expect(kwestia("zrobione", wstawki, kwestie, () => 0)).toBe("Brawo! Montaż odhaczone.");
    expect(kwestia("zrobione", wstawki, kwestie, () => 0.99)).toBe("Mamy to! 47 min roboty.");
  });
});

describe("kwestie.json a backend", () => {
  const rdzen = readFileSync(new URL("../src-tauri/src/rdzen.rs", import.meta.url), "utf8");
  // Testy w Ruście też wołają dymki; liczy się tylko kod przed modułem testów.
  const kod = rdzen.split("#[cfg(test)]")[0] ?? "";
  const klucze = [...kod.matchAll(/self\.dymek\("([a-z_]+)"/g)].map((m) => m[1]);

  it("ma wariant dla każdego dymka z rdzenia", () => {
    expect(klucze.length).toBeGreaterThan(10);
    for (const klucz of klucze) {
      expect(kwestie, `brak klucza ${klucz}`).toHaveProperty(klucz as string);
    }
  });

  it("używa tylko wstawek, które backend umie podać", () => {
    const znane = new Set(["imie", "dni", "minuty", "zadanie", "godzina_planu"]);
    for (const warianty of Object.values(kwestie)) {
      for (const wzor of warianty) {
        for (const [, nazwa] of wzor.matchAll(/\{(\w+)\}/g)) {
          expect(znane, `nieznana wstawka {${nazwa}} w „${wzor}”`).toContain(nazwa);
        }
      }
    }
  });
});
