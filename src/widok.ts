// Panele stanów z B4. Panel rysuje się na nowo przy każdej migawce, a licznik
// i pasek czasu aktualizuje `aktualizuj` raz na sekundę, bez przebudowy DOM.

import { godzinaBezZera, licznik, minutyTekst, naMinuty, szacunekTekst } from "./format";
import { W_TOKU, ZROBIONE, type Gratulacje, type Migawka, type SesjaWidok, type ZadanieWidok } from "./typy";

type Dziecko = Node | string | null | undefined | false;
type Atrybuty = Record<string, string | boolean | undefined>;

function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  atrybuty: Atrybuty = {},
  ...dzieci: (Dziecko | Dziecko[])[]
): HTMLElementTagNameMap[K] {
  const element = document.createElement(tag);
  for (const [nazwa, wartosc] of Object.entries(atrybuty)) {
    if (wartosc === true) element.setAttribute(nazwa, "");
    else if (typeof wartosc === "string") element.setAttribute(nazwa, wartosc);
  }
  for (const dziecko of dzieci.flat()) {
    if (dziecko === null || dziecko === undefined || dziecko === false) continue;
    element.append(dziecko);
  }
  return element;
}

const NAZWY_SCIEZEK: Record<string, string> = {
  folder_planow: "folder planów",
  plik_stanu: "plik stanu zadań",
  folder_dziennika: "folder dziennika",
};

function wierszZadania(z: ZadanieWidok): HTMLElement {
  const zrobione = z.status === ZROBIONE;
  const wToku = z.status === W_TOKU;
  const czas = zrobione
    ? "✓"
    : z.minuty_dzis > 0
      ? szacunekTekst(z.minuty_dzis, z.szac_min)
      : z.szac_min != null
        ? `${z.szac_min} min`
        : "";
  const klasy = ["zadanie", zrobione && "zrobione", wToku && "w-toku"].filter(Boolean).join(" ");
  return el(
    "li",
    {},
    el(
      "button",
      { class: klasy, "data-id": z.id, disabled: zrobione, type: "button" },
      el(
        "span",
        { class: "opis-zadania" },
        el("span", { class: "nazwa" }, z.nazwa),
        wToku && z.minuty_razem > 0 && el("span", { class: "znacznik" }, `w toku · ${minutyTekst(z.minuty_razem)}`),
      ),
      el("span", { class: "czas" }, czas),
    ),
  );
}

function panelListy(m: Migawka): HTMLElement {
  const plan = m.plan!;
  return el(
    "section",
    { class: "panel panel-lista" },
    el("header", { class: plan.zagrozone ? "naglowek zagrozone" : "naglowek" }, plan.naglowek || "Plan na dziś"),
    plan.zadania.length > 0
      ? el("ul", { class: "zadania" }, plan.zadania.map(wierszZadania))
      : el("p", { class: "opis" }, "Na dziś nie ma zadań w planie."),
    plan.zapasowe.length > 0 && [
      el("div", { class: "sekcja" }, "Zapasowe"),
      el("ul", { class: "zadania zapasowe" }, plan.zapasowe.map(wierszZadania)),
    ],
  );
}

function panelCzekania(m: Migawka): HTMLElement {
  return el(
    "section",
    { class: "panel panel-czekanie" },
    el("header", { class: "naglowek" }, `Plan będzie po ${godzinaBezZera(m.godzina_planu)}`),
    m.w_toku.length > 0
      ? [el("p", { class: "opis" }, "Możesz wrócić do zadań w toku:"), el("ul", { class: "zadania" }, m.w_toku.map(wierszZadania))]
      : el("p", { class: "opis" }, "Na razie nie ma zadań w toku."),
    m.brakujace_sciezki.length > 0 &&
      el(
        "p",
        { class: "ostrzezenie" },
        "Uzupełnij w ustawieniach (zasobnik → Ustawienia): ",
        m.brakujace_sciezki.map((s) => NAZWY_SCIEZEK[s] ?? s).join(", "),
        ".",
      ),
  );
}

function panelPracy(s: SesjaWidok, pauza: boolean): HTMLElement {
  return el(
    "section",
    { class: pauza ? "panel panel-praca pauza" : "panel panel-praca" },
    el("div", { class: "tytul", title: s.nazwa }, s.nazwa),
    el(
      "div",
      { class: "licznik-wiersz" },
      el("span", { class: "licznik", "data-licznik": true }),
      el("span", { class: "szacunek", "data-szacunek": true }),
    ),
    el("div", { class: "pasek" }, el("div", { class: "pasek-wypelnienie", "data-pasek": true })),
    pauza && el("div", { class: "czas-pauzy" }, "Pauza ", el("span", { "data-pauza": true })),
    el(
      "div",
      { class: "przyciski" },
      pauza
        ? el("button", { class: "przycisk glowny", "data-akcja": "wznow", type: "button" }, "Wznów")
        : [
            el("button", { class: "przycisk", "data-akcja": "pauza", type: "button" }, "Pauza"),
            el("button", { class: "przycisk glowny", "data-akcja": "zrobione", type: "button" }, "Zrobione"),
            el("button", { class: "przycisk cichy", "data-akcja": "przerwij", type: "button" }, "Przerwij"),
          ],
    ),
  );
}

function panelJestes(s: SesjaWidok): HTMLElement {
  return el(
    "section",
    { class: "panel panel-jestes" },
    el("div", { class: "tytul", title: s.nazwa }, s.nazwa),
    el("p", { class: "opis" }, "Bez ruchu od ", el("span", { "data-bezczynnosc": true }), ". Przerwa czy praca?"),
    el(
      "div",
      { class: "przyciski" },
      el("button", { class: "przycisk", "data-akcja": "przerwa", "data-odejmij": true, type: "button" }, "Przerwa"),
      el("button", { class: "przycisk glowny", "data-akcja": "pracowalem", type: "button" }, "Pracowałem"),
    ),
  );
}

function panelGratulacji(g: Gratulacje): HTMLElement {
  const szacunek = g.szac_min != null ? ` (szac. ${g.szac_min})` : "";
  return el(
    "section",
    { class: "panel panel-gratulacje" },
    el("div", { class: "tytul" }, `✓ ${g.nazwa}`),
    el("p", { class: "opis" }, `${g.minuty} min${szacunek} · zapisane w dzienniku`),
  );
}

function panelKonca(m: Migawka): HTMLElement {
  return el(
    "section",
    { class: "panel panel-koniec" },
    el("header", { class: "naglowek" }, "Podsumowanie dnia"),
    el("div", { class: "suma" }, m.dzien.minuty > 0 ? `${minutyTekst(m.dzien.minuty)} pracy` : "Dziś bez zapisanej pracy"),
    m.dzien.zrobione.length > 0 && el("ul", { class: "zrobione-lista" }, m.dzien.zrobione.map((n) => el("li", {}, `✓ ${n}`))),
    el("button", { class: "link", "data-akcja": "pokaz_liste", type: "button" }, "Wróć do listy"),
  );
}

/** Panel dla stanu; Powitanie ma sam dymek. */
export function panel(m: Migawka): HTMLElement | null {
  switch (m.stan) {
    case "powitanie":
      return null;
    case "lista":
      return m.plan ? panelListy(m) : panelCzekania(m);
    case "czeka_na_plan":
      return panelCzekania(m);
    case "praca":
    case "pauza":
      return m.sesja ? panelPracy(m.sesja, m.stan === "pauza") : null;
    case "jestes":
      return m.sesja ? panelJestes(m.sesja) : null;
    case "gratulacje":
      return m.gratulacje ? panelGratulacji(m.gratulacje) : null;
    case "koniec_dnia":
      return panelKonca(m);
  }
}

/** Sekundy aktywnej pracy w sesji: zamknięte odcinki plus trwający. */
export function sekundySesji(s: SesjaWidok, teraz: number): number {
  const trwa = s.odcinek_od_ms != null ? Math.max(0, (teraz - s.odcinek_od_ms) / 1000) : 0;
  return s.sekundy_zamkniete + trwa;
}

function ustawTekst(korzen: ParentNode, selektor: string, tekst: string): void {
  const element = korzen.querySelector(selektor);
  if (element && element.textContent !== tekst) element.textContent = tekst;
}

/** Licznik, szacunek, pasek czasu, czas pauzy i bezczynności. */
export function aktualizuj(korzen: ParentNode, m: Migawka, teraz: number): void {
  const s = m.sesja;
  if (!s) return;
  const sekundy = sekundySesji(s, teraz);
  const minutyDzis = s.minuty_wczesniej + naMinuty(sekundy);
  ustawTekst(korzen, "[data-licznik]", licznik(sekundy));
  ustawTekst(korzen, "[data-szacunek]", szacunekTekst(minutyDzis, s.szac_min));
  if (s.pauza_od_ms != null) ustawTekst(korzen, "[data-pauza]", licznik((teraz - s.pauza_od_ms) / 1000));
  if (s.bezczynna_od_ms != null) {
    const minuty = Math.max(1, Math.round((teraz - s.bezczynna_od_ms) / 60000));
    ustawTekst(korzen, "[data-bezczynnosc]", minutyTekst(minuty));
    ustawTekst(korzen, "[data-odejmij]", `Przerwa (odejmij ${minuty} min)`);
  }
  const pasek = korzen.querySelector<HTMLElement>("[data-pasek]");
  if (pasek) {
    const czesc = s.szac_min ? (s.minuty_wczesniej * 60 + sekundy) / (s.szac_min * 60) : 0;
    pasek.style.transform = `scaleX(${Math.min(1, czesc).toFixed(4)})`;
    // Po przekroczeniu szacunku pasek robi się brzoskwiniowy, bez komunikatów o winie.
    pasek.classList.toggle("ponad", czesc > 1);
  }
}
