import { readdirSync, readFileSync } from "node:fs";

import { describe, expect, it } from "vitest";

const odczyt = (nazwa: string) => readFileSync(new URL(`./${nazwa}`, import.meta.url), "utf8");

/** Zmienne z bloku :root w kolory.css. */
function paleta(): Record<string, string> {
  const wynik: Record<string, string> = {};
  for (const [, nazwa, wartosc] of odczyt("kolory.css").matchAll(/--([\w-]+):\s*(#[0-9a-fA-F]{6})\s*;/g)) {
    wynik[nazwa as string] = (wartosc as string).toUpperCase();
  }
  return wynik;
}

function jasnosc(hex: string): number {
  const kanaly = [1, 3, 5].map((i) => Number.parseInt(hex.slice(i, i + 2), 16) / 255);
  const [r = 0, g = 0, b = 0] = kanaly.map((c) => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

function kontrast(a: string, b: string): number {
  const [jasny, ciemny] = [jasnosc(a), jasnosc(b)].sort((x, y) => y - x) as [number, number];
  return (jasny + 0.05) / (ciemny + 0.05);
}

describe("paleta interfejsu (B7)", () => {
  const p = paleta();

  it("ma dokładnie zmienne i wartości z B7, w jednym miejscu", () => {
    expect(p).toEqual({
      tlo: "#101B17",
      "tlo-2": "#1A2B24",
      ramka: "#41725F",
      tekst: "#FCF0DF",
      "tekst-2": "#B3C2B8",
      miedz: "#C7773F",
      "miedz-ciemna": "#893E25",
      fiolet: "#A778C8",
      "fiolet-jasny": "#CDB4EE",
      zielen: "#7CC49E",
      koral: "#FD8F72",
      dymek: "#FCF0DF",
      "dymek-tekst": "#101B17",
    });
  });

  it("spełnia kontrasty podane w B7 (WCAG AA)", () => {
    const k = (a: string, b: string) => kontrast(p[a] as string, p[b] as string);
    // tekst i liczby na tle panelu i na kafelkach --tlo-2: co najmniej 6,6:1
    for (const tekst of ["tekst", "tekst-2", "fiolet-jasny", "koral", "zielen"]) {
      for (const tlo of ["tlo", "tlo-2"]) {
        expect(k(tekst, tlo), `${tekst} na ${tlo}`).toBeGreaterThanOrEqual(6.6);
      }
    }
    expect(k("dymek-tekst", "dymek")).toBeGreaterThanOrEqual(6.6);
    // ramka i pasek: co najmniej 3:1
    expect(k("ramka", "tlo")).toBeGreaterThanOrEqual(3);
    expect(k("fiolet", "tlo-2")).toBeGreaterThanOrEqual(3);
    // przycisk główny: ciemny tekst na miedzi ma ponad 5:1, a kremowy tylko 3:1, więc go nie używamy
    expect(k("tlo", "miedz")).toBeGreaterThan(5);
    expect(k("tekst", "miedz")).toBeLessThan(3.1);
    // po wciśnięciu: kremowy tekst na --miedz-ciemna
    expect(k("tekst", "miedz-ciemna")).toBeGreaterThanOrEqual(4.5);
  });
});

describe("komponenty biorą kolory tylko ze zmiennych", () => {
  const pliki = readdirSync(new URL(".", import.meta.url)).filter((n) => n.endsWith(".css") && n !== "kolory.css");
  const p = paleta();
  const uklad = new Set(["postac-w", "postac-h", "pod-siedziskiem", "pod-stopami", "panel-w", "oddech-max", "panel-max-h"]);
  const wlasne = new Set(["x", "dx", "dy", "obrot", "czas", "opoznienie", "kolor", "klatka-czas"]);

  it("znajduje arkusze stylów", () => {
    expect(pliki).toEqual(expect.arrayContaining(["style.css", "formularz.css", "ustawienia.css"]));
  });

  for (const plik of pliki) {
    it(`${plik}: żadnych kolorów wpisanych na sztywno`, () => {
      const css = odczyt(plik).replace(/\/\*[\s\S]*?\*\//g, "");
      expect(css.match(/#[0-9a-fA-F]{3,8}\b/g) ?? []).toEqual([]);
      expect(css.match(/\b(?:rgb|rgba|hsl|hsla|hwb|lab|lch|oklab|oklch)\(/g) ?? []).toEqual([]);
    });

    it(`${plik}: używa tylko zmiennych z palety albo z układu`, () => {
      const css = odczyt(plik).replace(/\/\*[\s\S]*?\*\//g, "");
      for (const [, nazwa] of css.matchAll(/var\(--([\w-]+)/g)) {
        const znana = nazwa as string;
        expect(znana in p || uklad.has(znana) || wlasne.has(znana), `nieznana zmienna --${znana}`).toBe(true);
      }
    });
  }

  it("stara paleta z A2 (granat, cyjan, lawenda, żółć) nie zostaje w kodzie interfejsu", () => {
    const stare = ["#B9A6FF", "#FFB59A", "#FFD84D", "#2EDBE8", "#1E2140"];
    for (const plik of [...pliki, "efekty.ts", "widok.ts", "main.ts", "ustawienia.ts", "kreator.ts", "dodawanie.ts"]) {
      const tresc = odczyt(plik).toUpperCase();
      for (const kolor of stare) expect(tresc, `${kolor} w ${plik}`).not.toContain(kolor);
    }
  });

  it("konfetti bierze kolory ze zmiennych: miedź, fiolet, zieleń i krem", () => {
    const efekty = odczyt("efekty.ts");
    expect(efekty).not.toMatch(/#[0-9a-fA-F]{3,8}\b/);
    for (const zmienna of ["--miedz", "--fiolet", "--zielen", "--tekst"]) expect(efekty).toContain(`var(${zmienna})`);
  });
});
