import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { Grafiki, Poza } from "./grafiki";
import { Postac } from "./postac";
import type { Migawka, NazwaStanu } from "./typy";

// Postać bez przeglądarki: tyle DOM, ile używa `Postac` (klasy, src, dataset, style).

class Klasy {
  private readonly zbior = new Set<string>();
  add(...nazwy: string[]) {
    for (const n of nazwy) this.zbior.add(n);
  }
  remove(...nazwy: string[]) {
    for (const n of nazwy) this.zbior.delete(n);
  }
  toggle(nazwa: string, wlacz?: boolean) {
    if (wlacz ?? !this.zbior.has(nazwa)) this.zbior.add(nazwa);
    else this.zbior.delete(nazwa);
  }
  contains(nazwa: string) {
    return this.zbior.has(nazwa);
  }
}

class Element {
  classList = new Klasy();
  dataset: Record<string, string> = {};
  style = { setProperty: () => undefined };
  offsetWidth = 0;
  src = "";
  /** Ile razy podmieniono obrazek. */
  podmiany = 0;
  constructor(private readonly dzieci: Record<string, Element> = {}) {
    let src = "";
    Object.defineProperty(this, "src", {
      get: () => src,
      set: (v: string) => {
        src = v;
        this.podmiany++;
      },
    });
  }
  querySelector(selektor: string) {
    return this.dzieci[selektor] ?? null;
  }
  removeAttribute() {
    this.src = "";
  }
}

const grafiki = {
  wszystkie: () => [],
  ma: () => true,
  adres: (p: Poza) => `${p}.png`,
  reakcje: {},
} as unknown as Grafiki;

function migawka(stan: NazwaStanu, pauzaOd: number | null = null): Migawka {
  return {
    stan,
    widoczna: true,
    sesja: stan === "praca" || stan === "pauza" ? { pauza_od_ms: pauzaOd } : null,
    koniec: null,
  } as unknown as Migawka;
}

function nowa() {
  const dom = {
    cialo: new Element(),
    skok: new Element(),
    a: new Element(),
    b: new Element(),
  };
  const korzen = new Element({ ".cialo": dom.cialo, ".skok": dom.skok, ".klatka-a": dom.a, ".klatka-b": dom.b });
  const postac = new Postac(korzen as unknown as HTMLElement, grafiki);
  return { postac, ...dom };
}

const MIN = 60_000;

beforeEach(() => {
  vi.useFakeTimers();
  vi.stubGlobal("window", globalThis);
  vi.stubGlobal(
    "Image",
    class {
      src = "";
    },
  );
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe("ukryte okno", () => {
  it("w Pracy schowanie zatrzymuje wszystkie zegary postaci", () => {
    const { postac, a } = nowa();
    postac.ustaw(migawka("praca"));
    postac.przeciagnijSie();
    expect(vi.getTimerCount()).toBeGreaterThan(0);
    postac.widocznosc(false);
    expect(vi.getTimerCount()).toBe(0);
    const podmiany = a.podmiany;
    vi.advanceTimersByTime(60 * MIN);
    expect(a.podmiany, "przez godzinę gry nic nie podmienia obrazków").toBe(podmiany);
  });

  it("w przerwie schowanie zatrzymuje fazy i konsolkę, a migawki w trakcie nie budzą zegarów", () => {
    const { postac } = nowa();
    postac.ustaw(migawka("pauza", Date.now()));
    vi.advanceTimersByTime(10_000);
    postac.widocznosc(false);
    postac.ustaw(migawka("praca"));
    postac.ustaw(migawka("pauza", Date.now()));
    postac.kursor(true);
    expect(vi.getTimerCount()).toBe(0);
  });

  it("po pokazaniu poza jest właściwa dla czasu przerwy, a zegary działają od nowa", () => {
    const { postac, a } = nowa();
    postac.ustaw(migawka("pauza", Date.now()));
    expect(a.dataset.poza).toBe("pause_stretch");
    postac.widocznosc(false);
    vi.advanceTimersByTime(20 * MIN);
    postac.widocznosc(true);
    expect(a.dataset.poza, "po 20 min przerwy od razu drzemka").toBe("pause_nap");
    postac.ustaw(migawka("praca"));
    expect(vi.getTimerCount(), "zmiana pozy i mruganie zaplanowane").toBeGreaterThan(0);
  });

  it("schowanie w trakcie zanikania nie zostawia postaci wygaszonej", () => {
    const { postac, cialo, a } = nowa();
    postac.ustaw(migawka("praca"));
    postac.kursor(true);
    expect(cialo.classList.contains("zanik")).toBe(true);
    postac.widocznosc(false);
    postac.widocznosc(true);
    expect(cialo.classList.contains("zanik")).toBe(false);
    expect(a.dataset.poza).toBe("sit_watch_front");
  });
});

describe("zegary bez uchwytów", () => {
  it("koniec przeciągania sprzed zmiany stanu nie skraca nowego przeciągania", () => {
    const { postac, a } = nowa();
    postac.ustaw(migawka("praca"));
    postac.przeciagnijSie(); // t = 0, koniec o 4000
    vi.advanceTimersByTime(1_000);
    postac.ustaw(migawka("jestes"));
    postac.ustaw(migawka("praca"));
    vi.advanceTimersByTime(1_000);
    postac.przeciagnijSie(); // t = 2000, koniec o 6000
    vi.advanceTimersByTime(2_100); // t = 4100
    expect(a.dataset.poza, "stary zegar nie kończy nowego przeciągania").toBe("pause_stretch");
    vi.advanceTimersByTime(2_000); // t = 6100
    expect(a.dataset.poza).not.toBe("pause_stretch");
  });
});

describe("zanikanie (Z-19)", () => {
  it("unieważnione zanikanie nie rozjaśnia postaci ze starą pozą", () => {
    const { postac, cialo, a } = nowa();
    postac.ustaw(migawka("praca"));
    expect(a.dataset.poza).toBe("sit_watch");
    postac.kursor(true); // t = 0: zmiana 1
    vi.advanceTimersByTime(40);
    postac.kursor(false); // t = 40: zmiana 2, wraca do sit_watch
    vi.advanceTimersByTime(36); // t = 76: minął zegar zmiany 1
    expect(cialo.classList.contains("zanik"), "dalej wygaszona do końca zmiany 2").toBe(true);
    vi.advanceTimersByTime(40); // t = 116: zmiana 2 zastosowana
    expect(cialo.classList.contains("zanik")).toBe(false);
    expect(a.dataset.poza).toBe("sit_watch");
  });
});

describe("spojrzenie na kursor w Pracy", () => {
  /** Zmiana pozy po 3–6 min: `Math.random` wskazuje konsolkę (0) albo kubek (0,99). */
  function zmienPoze(wybor: number) {
    vi.spyOn(Math, "random").mockReturnValue(wybor);
    vi.advanceTimersByTime(6 * MIN + 200);
    vi.restoreAllMocks();
  }

  it("przy patrzeniu kursor blisko daje sit_watch_front", () => {
    const { postac, a } = nowa();
    postac.ustaw(migawka("praca"));
    postac.kursor(true);
    vi.advanceTimersByTime(200);
    expect(a.dataset.poza).toBe("sit_watch_front");
  });

  it("nie przerywa konsolki ani kubka", () => {
    for (const [wybor, poza] of [
      [0, "sit_console_1"],
      [0.99, "sit_drink"],
    ] as const) {
      const { postac, a } = nowa();
      postac.ustaw(migawka("praca"));
      zmienPoze(wybor);
      expect(a.dataset.poza).toBe(poza);
      postac.kursor(true);
      vi.advanceTimersByTime(200);
      expect(a.dataset.poza, `kursor przy ${poza}`).toBe(poza);
    }
  });
});
