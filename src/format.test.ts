import { describe, expect, it } from "vitest";

import { godzinaBezZera, licznik, minutyTekst, naMinuty, szacunekTekst } from "./format";

describe("licznik", () => {
  it("pokazuje mm:ss, a od godziny h:mm:ss", () => {
    expect(licznik(0)).toBe("00:00");
    expect(licznik(47 * 60 + 12)).toBe("47:12");
    expect(licznik(3599.9)).toBe("59:59");
    expect(licznik(3600)).toBe("1:00:00");
    expect(licznik(2 * 3600 + 5 * 60 + 9)).toBe("2:05:09");
    expect(licznik(-3)).toBe("00:00");
  });
});

describe("minuty jak w dzienniku", () => {
  it("formatuje i zaokrągla tak samo jak backend", () => {
    expect(minutyTekst(47)).toBe("47 min");
    expect(minutyTekst(132)).toBe("2 h 12 min");
    expect(minutyTekst(60)).toBe("1 h");
    expect(naMinuty(29)).toBe(0);
    expect(naMinuty(30)).toBe(1);
    expect(naMinuty(47 * 60 + 29)).toBe(47);
  });

  it("szacunek obok licznika", () => {
    expect(szacunekTekst(47, 60)).toBe("47 / 60 min");
    expect(szacunekTekst(47, null)).toBe("47 min");
  });

  it("godzina planu bez zera wiodącego", () => {
    expect(godzinaBezZera("07:40")).toBe("7:40");
    expect(godzinaBezZera("22:00")).toBe("22:00");
  });
});

describe("termin", () => {
  it("pokazuje dzień i miesiąc", async () => {
    const { terminTekst } = await import("./format");
    expect(terminTekst("2026-11-05")).toBe("5.11");
    expect(terminTekst("2026-12-31")).toBe("31.12");
    expect(terminTekst("jutro")).toBe("jutro");
  });
});
