import { describe, expect, it } from "vitest";

import { Grafiki, POZY, ZAMIENNIKI } from "./grafiki";

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
