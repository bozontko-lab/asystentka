import { describe, expect, it } from "vitest";

import { szacunekZPola } from "./dodawanie";

describe("szacunek w formularzu", () => {
  it("puste i zero to brak szacunku, liczba to minuty, reszta to błąd", () => {
    expect(szacunekZPola("")).toBeNull();
    expect(szacunekZPola("  ")).toBeNull();
    expect(szacunekZPola("0")).toBeNull();
    expect(szacunekZPola("45")).toBe(45);
    expect(szacunekZPola(" 120 ")).toBe(120);
    expect(szacunekZPola("1,5")).toBe("blad");
    expect(szacunekZPola("abc")).toBe("blad");
    expect(szacunekZPola("-5")).toBe("blad");
  });
});
