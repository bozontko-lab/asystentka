// Formularz „Dodaj zadanie” (SPEC.md, B4): nazwa, szacunek i projekt. Widget
// dopisuje linię pod wybranym nagłówkiem i od razu dodaje zadanie do planu.

import { el } from "./dom";
import type { Most } from "./most";

export interface OpcjeDodawania {
  /** Dopisane albo anulowane: formularz znika. */
  poZamknieciu(): void;
}

/** Szacunek z pola: puste to brak szacunku, a liczba całkowita dodatnia to minuty. */
export function szacunekZPola(tekst: string): number | null | "blad" {
  const t = tekst.trim();
  if (t === "") return null;
  if (!/^\d{1,4}$/.test(t)) return "blad";
  const n = Number(t);
  return n > 0 ? n : null;
}

export class Dodawanie {
  readonly korzen = el("section", { class: "panel panel-dodawanie" });
  private blad = "";
  private zajety = false;

  constructor(
    private readonly most: Most,
    private readonly projekty: string[],
    private readonly opcje: OpcjeDodawania,
  ) {
    this.render();
  }

  private render(): void {
    const nazwa = el("input", { id: "dodaj-nazwa", type: "text", spellcheck: "false", placeholder: "np. Napisać opis filmu" });
    const szac = el("input", { id: "dodaj-szac", type: "text", inputmode: "numeric", placeholder: "minuty" });
    const projekt = el(
      "select",
      { id: "dodaj-projekt" },
      this.projekty.map((p) => el("option", { value: p }, p)),
    );
    projekt.value = this.projekty.includes("Inbox") ? "Inbox" : (this.projekty[0] ?? "");
    const wyslij = () => void this.dodaj(nazwa.value, szac.value, projekt.value);
    for (const w of [nazwa, szac]) {
      w.addEventListener("keydown", (e) => {
        if (e.key === "Enter") wyslij();
        if (e.key === "Escape") this.opcje.poZamknieciu();
      });
    }
    const dodaj = el("button", { class: "przycisk glowny", type: "button", disabled: this.zajety }, "Dodaj");
    dodaj.addEventListener("click", wyslij);
    const anuluj = el("button", { class: "przycisk cichy", type: "button" }, "Anuluj");
    anuluj.addEventListener("click", () => this.opcje.poZamknieciu());
    this.korzen.replaceChildren(
      el("header", { class: "naglowek" }, "Dodaj zadanie"),
      el("div", { class: "pole" }, el("label", { for: "dodaj-nazwa" }, "Nazwa"), nazwa),
      el(
        "div",
        { class: "pole" },
        el("label", { for: "dodaj-szac" }, "Szacunek (min)"),
        szac,
        el("span", { class: "podpowiedz" }, "Bez szacunku zadanie liczy się w sumie jako 0."),
      ),
      el("div", { class: "pole" }, el("label", { for: "dodaj-projekt" }, "Projekt"), projekt),
      el("div", { class: "blad-formularza", role: "alert", hidden: this.blad === "" }, this.blad),
      el("div", { class: "przyciski" }, anuluj, dodaj),
    );
    nazwa.focus();
  }

  private async dodaj(nazwa: string, szac: string, projekt: string): Promise<void> {
    if (this.zajety) return;
    if (nazwa.trim() === "") return this.pokazBlad("Wpisz nazwę zadania.");
    const szacunek = szacunekZPola(szac);
    if (szacunek === "blad") return this.pokazBlad("Szacunek to liczba minut, np. 45.");
    this.zajety = true;
    try {
      await this.most.dodajZadanie(nazwa.trim(), szacunek, projekt);
      this.opcje.poZamknieciu();
    } catch (e) {
      this.zajety = false;
      this.pokazBlad(typeof e === "string" ? e : e instanceof Error ? e.message : "Nie udało się dopisać zadania.");
    }
  }

  private pokazBlad(tresc: string): void {
    this.blad = tresc;
    const nazwa = (document.getElementById("dodaj-nazwa") as HTMLInputElement | null)?.value ?? "";
    const szac = (document.getElementById("dodaj-szac") as HTMLInputElement | null)?.value ?? "";
    const projekt = (document.getElementById("dodaj-projekt") as HTMLSelectElement | null)?.value ?? "";
    this.render();
    (document.getElementById("dodaj-nazwa") as HTMLInputElement).value = nazwa;
    (document.getElementById("dodaj-szac") as HTMLInputElement).value = szac;
    if (projekt) (document.getElementById("dodaj-projekt") as HTMLSelectElement).value = projekt;
  }
}
