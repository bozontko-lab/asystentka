import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { CZAS_DYMKU_COFNIJ_MS, CZAS_DYMKU_MS, Dymek } from "./dymki";

// Klasa `Dymek` bez przeglądarki: tyle DOM, ile używa (tekst, przycisk, kliknięcia).

type Obsluga = (z: { stopPropagation: () => void }) => void;

class Element {
  hidden = true;
  offsetWidth = 0;
  type = "";
  className = "";
  rodzic: Element | null = null;
  dzieci: Element[] = [];
  private tekst = "";
  private readonly obslugi: Obsluga[] = [];
  readonly classList = { add: () => undefined, remove: () => undefined };
  get textContent() {
    return this.tekst + this.dzieci.map((d) => d.textContent).join("");
  }
  set textContent(t: string) {
    this.tekst = t;
    this.dzieci = [];
  }
  addEventListener(_: string, f: Obsluga) {
    this.obslugi.push(f);
  }
  append(d: Element) {
    d.rodzic = this;
    this.dzieci.push(d);
  }
  remove() {
    if (this.rodzic) this.rodzic.dzieci = this.rodzic.dzieci.filter((d) => d !== this);
    this.rodzic = null;
  }
  /** Kliknięcie z bąbelkowaniem do rodzica, chyba że ktoś je zatrzyma. */
  kliknij() {
    let zatrzymane = false;
    const z = { stopPropagation: () => (zatrzymane = true) };
    for (const f of this.obslugi) f(z);
    if (!zatrzymane) this.rodzic?.kliknij();
  }
}

function nowy() {
  const element = new Element();
  const zmiany = vi.fn();
  const dymek = new Dymek(element as unknown as HTMLElement, zmiany);
  return { dymek, element, zmiany };
}

beforeEach(() => {
  vi.useFakeTimers();
  vi.stubGlobal("window", globalThis);
  vi.stubGlobal("document", { createElement: () => new Element() });
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe("Dymek", () => {
  it("znika po 6 s", () => {
    const { dymek, element } = nowy();
    dymek.pokaz("Dobra, pilnuję czasu.");
    expect(element.hidden).toBe(false);
    vi.advanceTimersByTime(CZAS_DYMKU_MS - 1);
    expect(element.hidden).toBe(false);
    vi.advanceTimersByTime(1);
    expect(element.hidden).toBe(true);
    expect(element.textContent).toBe("");
  });

  it("dymek gratulacji z Cofnij trwa 10 s", () => {
    const { dymek, element } = nowy();
    dymek.pokaz("Brawo!", { czasMs: CZAS_DYMKU_COFNIJ_MS, przycisk: { etykieta: "Cofnij", poKliknieciu: () => undefined } });
    expect(element.textContent).toBe("Brawo!Cofnij");
    vi.advanceTimersByTime(CZAS_DYMKU_MS);
    expect(element.hidden).toBe(false);
    vi.advanceTimersByTime(CZAS_DYMKU_COFNIJ_MS - CZAS_DYMKU_MS);
    expect(element.hidden).toBe(true);
  });

  it("kliknięcie w Cofnij woła tylko Cofnij, a kliknięcie w tło tylko akcję dymka", () => {
    const { dymek, element } = nowy();
    const cofnij = vi.fn();
    const dalej = vi.fn();
    dymek.pokaz("Brawo!", { przycisk: { etykieta: "Cofnij", poKliknieciu: cofnij }, poKliknieciu: dalej });
    element.dzieci[0]?.kliknij();
    expect(cofnij).toHaveBeenCalledTimes(1);
    expect(dalej).not.toHaveBeenCalled();
    expect(element.hidden).toBe(true);

    dymek.pokaz("Brawo!", { przycisk: { etykieta: "Cofnij", poKliknieciu: cofnij }, poKliknieciu: dalej });
    element.kliknij();
    expect(dalej).toHaveBeenCalledTimes(1);
    expect(cofnij).toHaveBeenCalledTimes(1);
    expect(element.hidden).toBe(true);
  });

  it("usunPrzycisk zabiera Cofnij, a dymek zostaje do końca swojego czasu", () => {
    const { dymek, element, zmiany } = nowy();
    dymek.pokaz("Brawo!", { czasMs: CZAS_DYMKU_COFNIJ_MS, przycisk: { etykieta: "Cofnij", poKliknieciu: () => undefined } });
    zmiany.mockClear();
    dymek.usunPrzycisk();
    expect(element.textContent).toBe("Brawo!");
    expect(element.hidden).toBe(false);
    expect(zmiany).toHaveBeenCalledTimes(1);
    dymek.usunPrzycisk();
    expect(zmiany, "drugi raz nic się nie dzieje").toHaveBeenCalledTimes(1);
  });

  it("nowy dymek zastępuje poprzedni i liczy czas od nowa", () => {
    const { dymek, element } = nowy();
    dymek.pokaz("Pierwszy");
    vi.advanceTimersByTime(5000);
    dymek.pokaz("Drugi");
    vi.advanceTimersByTime(5000);
    expect(element.hidden).toBe(false);
    expect(element.textContent).toBe("Drugi");
    vi.advanceTimersByTime(1000);
    expect(element.hidden).toBe(true);
  });

  it("kliknięcie po schowaniu nie woła starej akcji", () => {
    const { dymek, element } = nowy();
    const dalej = vi.fn();
    dymek.pokaz("Brawo!", { poKliknieciu: dalej });
    vi.advanceTimersByTime(CZAS_DYMKU_MS);
    element.kliknij();
    expect(dalej).not.toHaveBeenCalled();
  });
});
