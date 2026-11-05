import { readdirSync, readFileSync } from "node:fs";

import { describe, expect, it } from "vitest";

// Reguły dla całego kodu frontendu, sprawdzane w źródłach (Z-17, Z-21).

const katalog = new URL(".", import.meta.url);
const pliki = readdirSync(katalog).filter((n) => n.endsWith(".ts") && !n.endsWith(".test.ts"));
const odczyt = (nazwa: string) => readFileSync(new URL(`./${nazwa}`, katalog), "utf8");

describe("źródła frontendu", () => {
  it("znajduje pliki", () => {
    expect(pliki).toEqual(expect.arrayContaining(["main.ts", "widok.ts", "ustawienia.ts", "kreator.ts"]));
  });

  it("styl ustawia się przez CSSOM, nie atrybutem `style` (CSP style-src)", () => {
    const zle = pliki.filter((p) => /\bstyle:\s*[`"']/.test(odczyt(p)));
    expect(zle).toEqual([]);
  });

  it("żadna obietnica z mostu nie jest porzucona bez obsługi błędu", () => {
    const zle: string[] = [];
    for (const plik of pliki) {
      for (const [nr, linia] of odczyt(plik).split("\n").entries()) {
        if (/\bvoid\s+(?:this\.)?(?:most\.\w+|invoke)\(/.test(linia) && !linia.includes(".catch(")) {
          zle.push(`${plik}:${nr + 1}: ${linia.trim()}`);
        }
      }
    }
    expect(zle).toEqual([]);
  });

  it("oba okna zgłaszają nieobsłużone błędy do logu", () => {
    for (const plik of ["main.ts", "ustawienia.ts"]) {
      const kod = odczyt(plik);
      expect(kod, plik).toContain('addEventListener("error"');
      expect(kod, plik).toContain('addEventListener("unhandledrejection"');
    }
  });
});
