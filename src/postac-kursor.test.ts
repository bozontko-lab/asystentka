import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { Grafiki, Poza } from "./grafiki";
import { Postac } from "./postac";
import type { Migawka } from "./typy";

// Spojrzenie na kursor w Pracy (decyzja właściciela do B7): przerywa tylko patrzenie.
// Tyle DOM, ile używa `Postac`.

function element(dzieci: Record<string, unknown> = {}) {
  const klasy = new Set<string>();
  return {
    classList: {
      add: (...n: string[]) => n.forEach((k) => klasy.add(k)),
      remove: (...n: string[]) => n.forEach((k) => klasy.delete(k)),
      toggle: (k: string, wlacz?: boolean) => ((wlacz ?? !klasy.has(k)) ? klasy.add(k) : klasy.delete(k)),
      contains: (k: string) => klasy.has(k),
    },
    dataset: {} as Record<string, string>,
    style: { setProperty: () => undefined },
    offsetWidth: 0,
    src: "",
    removeAttribute() {
      this.src = "";
    },
    querySelector: (s: string) => dzieci[s] ?? null,
  };
}

const grafiki = {
  wszystkie: () => [],
  ma: () => true,
  adres: (p: Poza) => `${p}.png`,
  reakcje: {},
} as unknown as Grafiki;

const praca = { stan: "praca", widoczna: true, sesja: { pauza_od_ms: null }, koniec: null } as unknown as Migawka;

function wPracy() {
  const a = element();
  const korzen = element({ ".cialo": element(), ".skok": element(), ".klatka-a": a, ".klatka-b": element() });
  const postac = new Postac(korzen as unknown as HTMLElement, grafiki);
  postac.ustaw(praca);
  return { postac, a };
}

/** Losowanie wariantu: `Math.random` wskazuje konsolkę albo kubek z listy innych niż „patrzy”. */
function zmienPoze(wybor: number) {
  vi.spyOn(Math, "random").mockReturnValue(wybor);
  vi.advanceTimersByTime(6 * 60_000 + 200);
}

beforeEach(() => {
  vi.useFakeTimers();
  vi.stubGlobal("window", globalThis);
  vi.stubGlobal("Image", class {});
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe("spojrzenie na kursor w Pracy", () => {
  it("przy patrzeniu kursor blisko daje sit_watch_front", () => {
    const { postac, a } = wPracy();
    expect(a.dataset.poza).toBe("sit_watch");
    postac.kursor(true);
    vi.advanceTimersByTime(200);
    expect(a.dataset.poza).toBe("sit_watch_front");
  });

  it("nie przerywa konsolki", () => {
    const { postac, a } = wPracy();
    zmienPoze(0); // „konsola”
    expect(a.dataset.poza).toBe("sit_console_1");
    postac.kursor(true);
    vi.advanceTimersByTime(200);
    expect(a.dataset.poza).toBe("sit_console_1");
  });

  it("nie przerywa kubka", () => {
    const { postac, a } = wPracy();
    zmienPoze(0.99); // „kubek”
    expect(a.dataset.poza).toBe("sit_drink");
    postac.kursor(true);
    vi.advanceTimersByTime(200);
    expect(a.dataset.poza).toBe("sit_drink");
  });
});
