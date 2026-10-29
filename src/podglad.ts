// Podgląd w zwykłej przeglądarce (`npm run dev`, bez Tauri): przykładowe
// migawki wszystkich stanów do pracy nad wyglądem i do zrzutów ekranu.
// Parametry adresu: ?stan=praca&strona=lewa&dymek=start

import type { Akcja, Most } from "./most";
import type { Migawka, NazwaStanu, Zdarzenie } from "./typy";

const MINUTA = 60_000;

function przyklad(stan: NazwaStanu, strona: "prawa" | "lewa"): Migawka {
  const teraz = Date.now();
  const plan = {
    naglowek: "Film #1 — 6 dni do publikacji (5.11), zostało 26 h → 4,5 h dziennie",
    zagrozone: false,
    dni_do_publikacji: 6,
    zadania: [
      { id: "12", nazwa: "Scenariusz: intro", etap: "Scenariusz", szac_min: 60, status: "Zrobione", minuty_dzis: 47, minuty_razem: 47 },
      { id: "15", nazwa: "Montaż: plansze 1–5", etap: "Montaż", szac_min: 120, status: "W toku", minuty_dzis: 0, minuty_razem: 45 },
    ],
    zapasowe: [{ id: "18", nazwa: "Miniatura: szkic", etap: "Miniatura", szac_min: 30, status: null, minuty_dzis: 0, minuty_razem: 0 }],
  };
  const sesja = {
    id: "15",
    nazwa: "Montaż: plansze 1–5",
    szac_min: 120,
    sekundy_zamkniete: 41 * 60,
    odcinek_od_ms: stan === "praca" || stan === "jestes" ? teraz - 6 * MINUTA - 12_000 : null,
    pauza_od_ms: stan === "pauza" ? teraz - 3 * MINUTA - 4000 : null,
    powod_pauzy: stan === "pauza" ? ("reczna" as const) : null,
    bezczynna_od_ms: stan === "jestes" ? teraz - 16 * MINUTA : null,
    minuty_wczesniej: 0,
  };
  return {
    stan,
    widoczna: true,
    strona,
    imie: "Dżiordżio",
    plan: stan === "czeka_na_plan" ? null : plan,
    w_toku: [plan.zadania[1]!],
    sesja: ["praca", "pauza", "jestes"].includes(stan) ? sesja : null,
    gratulacje: stan === "gratulacje" ? { nazwa: "Scenariusz: intro", minuty: 47, szac_min: 60, ostatnie: false } : null,
    dzien: { minuty: 132, zrobione: ["Scenariusz: intro", "Miniatura: szkic"] },
    godzina_planu: "07:40",
    brakujace_sciezki: stan === "czeka_na_plan" ? ["folder_planow"] : [],
  };
}

const DYMKI: Partial<Record<NazwaStanu, Zdarzenie>> = {
  powitanie: { rodzaj: "dymek", klucz: "powitanie", wstawki: { imie: "Dżiordżio", dni: "6 dni" } },
  lista: { rodzaj: "dymek", klucz: "lista", wstawki: {} },
  czeka_na_plan: { rodzaj: "dymek", klucz: "czeka_na_plan", wstawki: { godzina_planu: "7:40" } },
  praca: { rodzaj: "dymek", klucz: "start", wstawki: { zadanie: "Montaż: plansze 1–5" } },
  pauza: { rodzaj: "dymek", klucz: "pauza", wstawki: {} },
  jestes: { rodzaj: "dymek", klucz: "jestes", wstawki: {} },
  gratulacje: { rodzaj: "dymek", klucz: "zrobione", wstawki: { zadanie: "Scenariusz: intro", minuty: "47 min" } },
  koniec_dnia: { rodzaj: "dymek", klucz: "koniec_dnia", wstawki: {} },
};

const STANY: NazwaStanu[] = ["powitanie", "lista", "czeka_na_plan", "praca", "pauza", "jestes", "gratulacje", "koniec_dnia"];

export function mostPodglad(): Most {
  const parametry = new URLSearchParams(location.search);
  const stan = (parametry.get("stan") as NazwaStanu | null) ?? "lista";
  const strona = parametry.get("strona") === "lewa" ? "lewa" : "prawa";
  const bezDymka = parametry.get("dymek") === "nie";
  const migawka = przyklad(STANY.includes(stan) ? stan : "lista", strona);
  let naZdarzenie: ((z: Zdarzenie) => void) | null = null;

  pasekPodgladu(stan, strona);
  return {
    migawka: async () => migawka,
    gotowy: async () => {
      const dymek = DYMKI[migawka.stan];
      if (dymek && !bezDymka) naZdarzenie?.(dymek);
      if (migawka.stan === "gratulacje") naZdarzenie?.({ rodzaj: "efekt", nazwa: "konfetti" });
    },
    dopasuj: async (szerokosc, wysokosc) => {
      document.documentElement.dataset.rozmiar = `${szerokosc}×${wysokosc}`;
    },
    rozpocznij: async (id) => console.info("rozpocznij", id),
    akcja: async (akcja: Akcja) => console.info("akcja", akcja),
    odpowiedzJestes: async (przerwa) => console.info("odpowiedź", przerwa),
    naMigawke: async () => undefined,
    naZdarzenie: async (obsluga) => {
      naZdarzenie = obsluga;
    },
    naKursor: async () => undefined,
    przeciagnij: () => undefined,
    zglosBlad: (tekst) => console.error(tekst),
  };
}

function pasekPodgladu(stan: NazwaStanu, strona: string): void {
  if (new URLSearchParams(location.search).has("czysto")) return;
  const pasek = document.createElement("nav");
  pasek.className = "podglad";
  for (const s of STANY) {
    const link = document.createElement("a");
    link.href = `?stan=${s}&strona=${strona}`;
    link.textContent = s;
    if (s === stan) link.setAttribute("aria-current", "page");
    pasek.append(link);
  }
  const lustro = document.createElement("a");
  lustro.href = `?stan=${stan}&strona=${strona === "lewa" ? "prawa" : "lewa"}`;
  lustro.textContent = "↔ strona";
  pasek.append(lustro);
  document.body.append(pasek);
  document.body.classList.add("tryb-podgladu");
}
