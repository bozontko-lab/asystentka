import { readFileSync } from "node:fs";

import { describe, expect, it } from "vitest";

import { kwestia } from "./dymki";
import kwestie from "./kwestie.json";

describe("kwestia", () => {
  it("podstawia wstawki z jednostkami", () => {
    const pierwszy = () => 0;
    expect(kwestia("powitanie", { imie: "Dżiordżio" }, kwestie, pierwszy)).toBe("Dzień dobry, Dżiordżio!");
    expect(kwestia("konfiguracja_gotowe", { imie: "Pawle" }, kwestie, pierwszy)).toBe(
      "Gotowe, Pawle! Dopisz zadania w pliku, a ja zaraz je wczytam.",
    );
  });

  it("pomija warianty z nieznanymi wstawkami", () => {
    // Bez {szac} zostaje drugi wariant „Brawo”, który go nie używa.
    const wstawki = { zadanie: "Miniatura", czas: "40 min", czas_dnia: "2 h 12 min" };
    expect(kwestia("zrobione", wstawki, kwestie, () => 0)).toBe("Mamy to! Miniatura w 40 min. Dziś już 2 h 12 min.");
    expect(kwestia("zrobione", { ...wstawki, szac: "30 min" }, kwestie, () => 0)).toBe(
      "Brawo! Miniatura: 40 min (szac. 30 min). Dziś razem 2 h 12 min.",
    );
    expect(kwestia("przypomnienie_przerwy", {}, kwestie)).toBeNull();
    expect(kwestia("nie_ma_takiego", {}, kwestie)).toBeNull();
  });

  it("losuje spośród pasujących wariantów", () => {
    expect(kwestia("powitanie", { imie: "Ola" }, kwestie, () => 0)).toBe("Dzień dobry, Ola!");
    expect(kwestia("powitanie", { imie: "Ola" }, kwestie, () => 0.99)).toBe("Hej, Ola! Co dziś robimy?");
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
    const znane = new Set(["imie", "minuty", "zadanie", "czas", "szac", "czas_dnia", "pozostalo", "zadan", "skrot"]);
    for (const warianty of Object.values(kwestie)) {
      for (const wzor of warianty) {
        for (const [, nazwa] of wzor.matchAll(/\{(\w+)\}/g)) {
          expect(znane, `nieznana wstawka {${nazwa}} w „${wzor}”`).toContain(nazwa);
        }
      }
    }
  });
});
