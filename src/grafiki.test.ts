import { describe, expect, it } from "vitest";

import manifest from "./assets/postac/postac.json";
import { GEOMETRIA_ZASTEPCZA, Grafiki, type Manifest, POZY, ZAMIENNIKI, geometriaZManifestu } from "./grafiki";

const PLIK = (nazwa: string) => `/assets/postac/${nazwa}.png`;
const OBOWIAZKOWE = POZY.filter((p) => !(p in ZAMIENNIKI));

describe("Grafiki", () => {
  it("bez PNG rysuje wszystko postacią zastępczą", () => {
    const g = new Grafiki(new Map());
    expect(g.zastepcza).toBe(true);
    for (const poza of POZY) {
      expect(g.adres(poza)).toMatch(/^data:image\/svg\+xml/);
      expect(g.ma(poza)).toBe(true);
    }
  });

  it("brakującą grafikę opcjonalną zastępuje poza z tej samej grupy", () => {
    const g = new Grafiki(new Map(OBOWIAZKOWE.map((p) => [p, PLIK(p)])));
    expect(g.zastepcza).toBe(false);
    expect(g.adres("wave_2")).toBe(PLIK("wave_1"));
    expect(g.adres("pause_nap")).toBe(PLIK("pause_stretch"));
    expect(g.adres("sit_drink")).toBe(PLIK("sit_watch"));
    expect(g.adres("loading")).toBe(PLIK("stand_idle"));
    expect(g.ma("wave_2")).toBe(false);
    expect(g.ma("wave_1")).toBe(true);
  });

  it("brakującą grafikę obowiązkową zastępuje postać zastępcza", () => {
    const g = new Grafiki(new Map([["stand_idle", PLIK("stand_idle")]]));
    expect(g.adres("stand_idle")).toBe(PLIK("stand_idle"));
    expect(g.adres("cheer_1")).toMatch(/^data:image\/svg\+xml/);
  });

  it("pakiet ma własną grafikę dla każdej pozy i opis płótna", () => {
    const g = new Grafiki();
    expect(g.zastepcza).toBe(false);
    for (const poza of POZY) {
      expect(g.ma(poza), `brak ${poza}.png`).toBe(true);
      expect(g.adres(poza)).not.toMatch(/^data:/);
      expect(manifest.pozy, `brak ${poza} w postac.json`).toHaveProperty(poza);
    }
  });

  it("wszystkie obowiązkowe pozy z A3 są w spisie", () => {
    expect(OBOWIAZKOWE).toEqual([
      "stand_idle",
      "stand_idle_blink",
      "wave_1",
      "sit_watch",
      "sit_watch_blink",
      "sit_console_1",
      "pause_stretch",
      "idle_check",
      "cheer_1",
      "bye",
    ]);
  });
});

describe("geometria postaci", () => {
  it("z postac.json: 228×280, siedzisko na krawędzi panelu, delikatny oddech", () => {
    const g = geometriaZManifestu(manifest);
    expect(g).toEqual({ szer: 228, wys: 280, podSiedziskiem: 96, podStopami: 4, oddech: 1.008 });
    expect(new Grafiki().geometria).toEqual(g);
  });

  it("bez linii z podstawowych póz bierze medianę, a bez opisu płótna wartości startowe", () => {
    const opis: Manifest = {
      plotno_px: [400, 400],
      wyswietlanie_css_px: [200, 200],
      pozy: { a: { siedzisko_y_px: 300 }, b: { siedzisko_y_px: 340 }, c: { stopy_y_px: 390 } },
    };
    expect(geometriaZManifestu(opis)).toEqual({ szer: 200, wys: 200, podSiedziskiem: 40, podStopami: 5, oddech: 1.015 });
    expect(geometriaZManifestu(undefined)).toEqual(GEOMETRIA_ZASTEPCZA);
    expect(new Grafiki(new Map()).geometria).toEqual(GEOMETRIA_ZASTEPCZA);
  });
});
