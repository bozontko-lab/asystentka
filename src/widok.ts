// Panele stanów z B4. Panel rysuje się na nowo przy każdej migawce, a licznik
// i pasek czasu aktualizuje `aktualizuj` raz na sekundę, bez przebudowy DOM.

import { el } from "./dom";
import { licznik, minutyTekst, naMinuty, szacunekTekst, terminTekst, zadanTekst } from "./format";
import type { Gratulacje, KoniecWidok, ListaWidok, Migawka, SesjaWidok, WyborWidok, ZadanieWidok } from "./typy";

const BEZ_PROJEKTU = "Bez projektu";

function ostrzezenie(tresc: string | null): HTMLElement | null {
  if (!tresc) return null;
  return el(
    "div",
    { class: "ostrzezenie" },
    tresc,
    " ",
    el("button", { class: "link", type: "button", "data-akcja": "otworz_ustawienia" }, "Ustawienia"),
  );
}

// ---- wybór zadań ---------------------------------------------------------------------------

function wierszWyboru(z: ZadanieWidok): HTMLElement {
  const wej = el("input", { type: "checkbox", "data-projekt": z.projekt, "data-zadanie": z.zadanie });
  wej.checked = z.zaznaczone;
  const meta: (string | false)[] = [
    z.szac_min != null ? `szac. ${minutyTekst(z.szac_min)}` : false,
    z.w_toku && `w toku · ${minutyTekst(z.czas_min)}`,
    z.termin != null && `termin ${terminTekst(z.termin)}`,
  ];
  return el(
    "li",
    {},
    el(
      "label",
      { class: z.w_toku ? "wiersz-wyboru w-toku" : "wiersz-wyboru" },
      wej,
      el(
        "span",
        { class: "opis-zadania" },
        el("span", { class: "nazwa" }, z.zadanie),
        el("span", { class: "znacznik" }, meta.filter(Boolean).join(" · ")),
      ),
      z.szac_min == null && el("span", { class: "bez-szacunku", title: "Zadanie bez szacunku liczy się w sumie jako 0" }, "bez szacunku"),
    ),
  );
}

export function sumaWyboru(w: Pick<WyborWidok, "suma_min" | "cel_min">): string {
  return `Razem ${minutyTekst(w.suma_min)} z celu ${minutyTekst(w.cel_min)}`;
}

function panelWyboru(w: WyborWidok, blad: string | null): HTMLElement {
  return el(
    "section",
    { class: "panel panel-wybor" },
    el("header", { class: "naglowek" }, "Wybierz zadania na dziś"),
    ostrzezenie(blad),
    el(
      "div",
      { class: "przewijany" },
      w.projekty.length === 0
        ? el("p", { class: "opis" }, "Nie ma otwartych zadań. Dopisz je w pliku albo przyciskiem poniżej.")
        : w.projekty.flatMap((p) => [
            el("div", { class: "sekcja" }, p.nazwa),
            el("ul", { class: "zadania" }, p.zadania.map(wierszWyboru)),
          ]),
    ),
    el("div", { class: w.przekroczony ? "suma-wyboru koral" : "suma-wyboru", "data-suma": true }, sumaWyboru(w)),
    el(
      "div",
      { class: "przyciski" },
      el("button", { class: "przycisk", type: "button", "data-akcja": "dodaj" }, "Dodaj zadanie"),
      w.mozna_wrocic && el("button", { class: "przycisk cichy", type: "button", "data-akcja": "anuluj_wybor" }, "Wróć"),
      el(
        "button",
        {
          class: "przycisk glowny",
          type: "button",
          "data-akcja": "zatwierdz_wybor",
          disabled: w.zaznaczonych === 0 && !w.mozna_wrocic,
        },
        "Zaczynamy",
      ),
    ),
  );
}

// ---- lista planu na dziś -------------------------------------------------------------------

function wierszListy(z: ZadanieWidok): HTMLElement {
  const czas = z.zrobione
    ? "✓"
    : z.czas_min > 0
      ? szacunekTekst(z.czas_min, z.szac_min)
      : z.szac_min != null
        ? `${z.szac_min} min`
        : "";
  const klasy = ["zadanie", z.zrobione && "zrobione", z.w_toku && "w-toku"].filter(Boolean).join(" ");
  return el(
    "li",
    {},
    el(
      "button",
      { class: klasy, "data-projekt": z.projekt, "data-zadanie": z.zadanie, disabled: z.zrobione, type: "button" },
      el(
        "span",
        { class: "opis-zadania" },
        el("span", { class: "nazwa" }, z.zadanie),
        z.w_toku && el("span", { class: "znacznik" }, `w toku · ${minutyTekst(z.czas_min)}`),
        z.projekt !== BEZ_PROJEKTU && !z.w_toku && el("span", { class: "znacznik cichy" }, z.projekt),
      ),
      el("span", { class: "czas" }, czas),
    ),
  );
}

function panelListy(l: ListaWidok, blad: string | null): HTMLElement {
  const stan = `${l.zrobione} z ${l.razem} zrobione`;
  return el(
    "section",
    { class: "panel panel-lista" },
    el("header", { class: "naglowek" }, "Plan na dziś"),
    el("p", { class: "opis" }, l.pozostalo_min > 0 ? `${stan} · zostało ok. ${minutyTekst(l.pozostalo_min)}` : stan),
    ostrzezenie(blad),
    el("div", { class: "przewijany" }, el("ul", { class: "zadania" }, l.zadania.map(wierszListy))),
    el(
      "div",
      { class: "przyciski" },
      el("button", { class: "przycisk", type: "button", "data-akcja": "zmien_plan" }, "Zmień plan"),
      el("button", { class: "przycisk", type: "button", "data-akcja": "dodaj" }, "Dodaj zadanie"),
    ),
  );
}

// ---- praca, pauza, pytanie, gratulacje -----------------------------------------------------------

function panelPracy(s: SesjaWidok, pauza: boolean): HTMLElement {
  return el(
    "section",
    { class: pauza ? "panel panel-praca pauza" : "panel panel-praca" },
    el("div", { class: "tytul", title: s.zadanie }, s.zadanie),
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
    el("div", { class: "tytul", title: s.zadanie }, s.zadanie),
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
  const szacunek = g.szac_min != null ? ` (szac. ${minutyTekst(g.szac_min)})` : "";
  return el(
    "section",
    { class: "panel panel-gratulacje" },
    el("div", { class: "tytul" }, `✓ ${g.zadanie}`),
    el("p", { class: "opis" }, `${minutyTekst(g.czas_min)}${szacunek} · dziś razem ${minutyTekst(g.czas_dnia_min)}`),
    // Po ostatnim zadaniu planu to Ty wybierasz, co dalej.
    g.plan_wykonany && [
      el("p", { class: "opis" }, "Plan na dziś wykonany."),
      el(
        "div",
        { class: "przyciski" },
        el("button", { class: "przycisk glowny", type: "button", "data-akcja": "koniec_pracy" }, "Koniec pracy"),
        el("button", { class: "przycisk", type: "button", "data-akcja": "zmien_plan" }, "Dodaj z listy"),
        el("button", { class: "przycisk cichy", type: "button", "data-akcja": "wroc_do_listy" }, "Wróć do listy"),
      ),
    ],
  );
}

// ---- koniec dnia -------------------------------------------------------------------------------

function wierszDnia(z: KoniecWidok["podsumowanie"]["zadania"][number]): HTMLElement {
  const szac = z.szac_min != null ? ` (szac. ${minutyTekst(z.szac_min)})` : "";
  return el(
    "li",
    { class: z.zrobione ? "zrobione-dnia" : "w-toku-dnia" },
    el("span", { class: "znak" }, z.zrobione ? "✓" : "•"),
    el("span", { class: "nazwa" }, z.zadanie),
    el("span", { class: "czas" }, `${minutyTekst(z.czas_min)}${szac}`),
  );
}

/** Podsumowanie dnia: zrobione z zaplanowanych, czas dnia względem celu, czas zadań i lista na jutro (B4). */
function panelKonca(k: KoniecWidok): HTMLElement {
  const p = k.podsumowanie;
  const wczoraj = k.etap === "wczoraj";
  const udzial = p.cel_min > 0 ? Math.min(1, p.czas_dnia_min / p.cel_min) : 0;
  return el(
    "section",
    { class: "panel panel-koniec" },
    el("header", { class: "naglowek" }, wczoraj ? "Podsumowanie wczorajszego dnia" : "Podsumowanie dnia"),
    el("div", { class: "suma" }, `Zrobione ${p.zrobione} z ${zadanTekst(p.zaplanowane)}`),
    el("div", { class: "czas-dnia" }, `${minutyTekst(p.czas_dnia_min)} z celu ${minutyTekst(p.cel_min)}`),
    el("div", { class: "pasek" }, el("div", { class: "pasek-wypelnienie", style: `transform: scaleX(${udzial.toFixed(3)})` })),
    el(
      "div",
      { class: "przewijany" },
      p.zadania.length > 0 && el("ul", { class: "zadania-dnia" }, p.zadania.map(wierszDnia)),
      p.na_jutro.length > 0 && [
        el("div", { class: "sekcja" }, "Na jutro"),
        el("ul", { class: "na-jutro" }, p.na_jutro.map((n) => el("li", {}, n))),
      ],
    ),
    el(
      "div",
      { class: "przyciski" },
      wczoraj
        ? el("button", { class: "przycisk glowny", type: "button", "data-akcja": "dalej_po_wczoraju" }, "Dalej")
        : [
            el("button", { class: "przycisk glowny", type: "button", "data-akcja": "do_jutra" }, "Do jutra"),
            el("button", { class: "przycisk", type: "button", "data-akcja": "wroc_do_pracy" }, "Wróć do pracy"),
          ],
    ),
  );
}

/** Panel dla stanu; Konfiguracja ma kreator, a Powitanie sam dymek. */
export function panel(m: Migawka): HTMLElement | null {
  switch (m.stan) {
    case "konfiguracja":
    case "powitanie":
      return null;
    case "wybor_zadan":
      return m.wybor ? panelWyboru(m.wybor, m.blad_pliku) : null;
    case "lista":
      return m.lista ? panelListy(m.lista, m.blad_pliku) : null;
    case "praca":
    case "pauza":
      return m.sesja ? panelPracy(m.sesja, m.stan === "pauza") : null;
    case "jestes":
      return m.sesja ? panelJestes(m.sesja) : null;
    case "gratulacje":
      return m.gratulacje ? panelGratulacji(m.gratulacje) : null;
    case "koniec_dnia":
      return m.koniec ? panelKonca(m.koniec) : null;
  }
}

/** Sekundy aktywnej pracy w sesji: czas z pliku, zamknięte odcinki i trwający. */
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
  ustawTekst(korzen, "[data-licznik]", licznik(sekundy));
  ustawTekst(korzen, "[data-szacunek]", szacunekTekst(naMinuty(sekundy), s.szac_min));
  if (s.pauza_od_ms != null) ustawTekst(korzen, "[data-pauza]", licznik((teraz - s.pauza_od_ms) / 1000));
  if (s.bezczynna_od_ms != null) {
    const minuty = Math.max(1, Math.round((teraz - s.bezczynna_od_ms) / 60000));
    ustawTekst(korzen, "[data-bezczynnosc]", minutyTekst(minuty));
    ustawTekst(korzen, "[data-odejmij]", `Przerwa (odejmij ${minuty} min)`);
  }
  const pasek = korzen.querySelector<HTMLElement>("[data-pasek]");
  if (pasek) {
    const czesc = s.szac_min ? sekundy / (s.szac_min * 60) : 0;
    pasek.style.transform = `scaleX(${Math.min(1, czesc).toFixed(4)})`;
    // Po przekroczeniu szacunku pasek robi się brzoskwiniowy, bez komunikatów o winie.
    pasek.classList.toggle("ponad", czesc > 1);
  }
}
