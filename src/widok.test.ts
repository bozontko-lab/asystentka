import { describe, expect, it } from "vitest";

import { oknoPrzyjmujeFokus } from "./widok";

describe("oknoPrzyjmujeFokus", () => {
  it("fokus tylko na czas kreatora albo formularza", () => {
    expect(oknoPrzyjmujeFokus(true, "konfiguracja", false)).toBe(true);
    expect(oknoPrzyjmujeFokus(true, "lista", true)).toBe(true);
    expect(oknoPrzyjmujeFokus(true, "lista", false)).toBe(false);
  });

  it("schowane okno nie przyjmuje fokusu, nawet z otwartym formularzem", () => {
    expect(oknoPrzyjmujeFokus(false, "lista", true)).toBe(false);
    expect(oknoPrzyjmujeFokus(false, "konfiguracja", false)).toBe(false);
  });
});
