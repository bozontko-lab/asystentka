// Dźwięk skupienia w ustawieniach (SPEC.md, B7): suwak głośności z przyciskiem „Odsłuchaj”
// na 5 s i podpowiedzią o słuchawkach. Pola wyboru „Dźwięk 40 Hz w czasie pracy” i „Cisza
// przy nagrywaniu (OBS)” budują ustawienia.ts.

import { el } from "./dom";
import type { Most } from "./most";

/** Odsłuch trwa tyle samo po stronie backendu (dzwiek.rs, `ODSLUCH`). */
export const ODSLUCH_MS = 5000;
/** Zmiany suwaka w trakcie odsłuchu dochodzą do dźwięku najwyżej co tyle. */
export const ODSTEP_SUWAKA_MS = 150;
export const DOMYSLNA_GLOSNOSC = 25;
const PODPOWIEDZ = "Najlepiej w słuchawkach: każde ucho dostaje inny ton, a głośniki dają zwykłe dudnienie.";

/** Głośność z pola suwaka: liczba całkowita od 0 do 100; coś innego daje domyślne 25. */
export function glosnoscZPola(tekst: string): number {
  const liczba = Number.parseInt(tekst, 10);
  if (Number.isNaN(liczba)) return DOMYSLNA_GLOSNOSC;
  return Math.min(100, Math.max(0, liczba));
}

type PrzyciskiMostu = Pick<Most, "dzwiekOdsluch">;

/** Wiersz „Głośność”: suwak, wartość, „Odsłuchaj” i podpowiedź, która w czasie odsłuchu mówi, że gra. */
export class PoleGlosnosci {
  readonly korzen: HTMLElement;
  private readonly suwak: HTMLInputElement;
  private readonly wartosc: HTMLElement;
  private readonly przycisk: HTMLButtonElement;
  private readonly wynik: HTMLElement;
  private odsluchDo = 0;
  private zegarKonca: number | undefined;
  private zegarSuwaka: number | undefined;

  constructor(
    private readonly most: PrzyciskiMostu,
    poczatkowa: number,
  ) {
    this.suwak = el("input", { id: "glosnosc", type: "range", min: "0", max: "100", step: "1" });
    this.suwak.value = String(glosnoscZPola(String(poczatkowa)));
    this.wartosc = el("output", { class: "wartosc-suwaka", for: "glosnosc" });
    this.przycisk = el("button", { class: "przycisk", type: "button" }, "Odsłuchaj");
    this.wynik = el("span", { class: "podpowiedz", role: "status" }, PODPOWIEDZ);
    this.korzen = el(
      "div",
      { class: "pole" },
      el("label", { for: "glosnosc" }, "Głośność dźwięku 40 Hz"),
      el("div", { class: "wiersz-glosnosci" }, this.suwak, this.wartosc, this.przycisk),
      this.wynik,
    );
    this.odswiezWartosc();
    this.suwak.addEventListener("input", () => {
      this.odswiezWartosc();
      this.poZmianieSuwaka();
    });
    this.przycisk.addEventListener("click", () => this.odsluchaj());
    window.addEventListener("pagehide", () => this.sprzatnij());
  }

  /** Głośność do zapisu w ustawieniach, od 0 do 100. */
  get glosnosc(): number {
    return glosnoscZPola(this.suwak.value);
  }

  private odswiezWartosc(): void {
    this.wartosc.textContent = `${this.glosnosc}%`;
    this.suwak.setAttribute("aria-valuetext", `${this.glosnosc} procent`);
  }

  private pokaz(komunikat: string, blad = false): void {
    this.wynik.textContent = komunikat;
    this.wynik.classList.toggle("blad-skrotu", blad);
    this.wynik.setAttribute("role", blad ? "alert" : "status");
  }

  private odsluchTrwa(): boolean {
    return Date.now() < this.odsluchDo;
  }

  private sprzatnij(): void {
    window.clearTimeout(this.zegarKonca);
    window.clearTimeout(this.zegarSuwaka);
  }

  private odsluchaj(): void {
    const poziom = this.glosnosc;
    this.most
      .dzwiekOdsluch(poziom)
      .then(() => {
        this.odsluchDo = Date.now() + ODSLUCH_MS;
        this.pokaz(poziom === 0 ? "Głośność 0: nic nie usłyszysz." : "Gra przez 5 s. Suwakiem możesz ją zmieniać.");
        window.clearTimeout(this.zegarKonca);
        this.zegarKonca = window.setTimeout(() => {
          if (!this.odsluchTrwa()) this.pokaz(PODPOWIEDZ);
        }, ODSLUCH_MS + 50);
      })
      .catch((blad: unknown) => {
        this.odsluchDo = 0;
        this.pokaz(typeof blad === "string" ? blad : "Nie mogę odtworzyć dźwięku.", true);
      });
  }

  /** W trakcie odsłuchu suwak zmienia głośność na żywo, żeby dało się ją dobrać na słuch. */
  private poZmianieSuwaka(): void {
    if (!this.odsluchTrwa() || this.zegarSuwaka !== undefined) return;
    this.zegarSuwaka = window.setTimeout(() => {
      this.zegarSuwaka = undefined;
      if (this.odsluchTrwa()) this.odsluchaj();
    }, ODSTEP_SUWAKA_MS);
  }
}
