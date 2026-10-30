// Podgląd w zwykłej przeglądarce (`npm run dev`, bez Tauri): przykładowe
// migawki wszystkich stanów do pracy nad wyglądem i do zrzutów ekranu.
// Parametry adresu: ?stan=praca&strona=lewa&dymek=start

import type { Akcja, Most } from "./most";
import type { Migawka, NazwaStanu, PodpowiedzKreatora, Reakcja, Ustawienia, ZadanieWidok, Zdarzenie } from "./typy";

const MINUTA = 60_000;

/** Ustawienia z wersji 1 w podglądzie: kreator ma je wypełnić. */
export const PODPOWIEDZ: PodpowiedzKreatora = {
  ustawienia: {
    imie: "Dżiordżio",
    plik_zadan: "D:\\Second Brain\\Zadania.md",
    folder_dziennika: "D:\\Second Brain\\Dziennik pracy",
    cel_dzienny_min: 240,
    godzina_konca_dnia: "22:00",
    autostart: true,
    prog_bezczynnosci_min: 15,
    przypomnienie_przerwy_min: 50,
    ukryj_przed_nagrywaniem: true,
    chowaj_przy_pelnym_ekranie: true,
    skrot_pokaz_ukryj: "Ctrl+Alt+Space",
  } satisfies Ustawienia,
  plik_istnieje: false,
  vaulty: ["D:\\Second Brain", "D:\\Notatki"],
};

function przyklad(stan: NazwaStanu, strona: "prawa" | "lewa"): Migawka {
  const teraz = Date.now();
  const z = (projekt: string, zadanie: string, szac: number | null, czas: number, opcje: Partial<ZadanieWidok> = {}): ZadanieWidok => ({
    projekt,
    zadanie,
    szac_min: szac,
    czas_min: czas,
    zrobione: false,
    w_toku: czas > 0,
    zaznaczone: false,
    termin: null,
    ...opcje,
  });
  const montaz = z("Film #1 · FIFA", "Montaż: plansze 1–5", 120, 45, { zaznaczone: true, termin: "2026-11-05" });
  const miniatura = z("Film #1 · FIFA", "Miniatura", 30, 0, { zaznaczone: true });
  const scenariusz = z("Film #1 · FIFA", "Scenariusz: intro", 60, 47, { zrobione: true, w_toku: false });
  const mail = z("Inbox", "Odpisać na maila od montażysty", 15, 0);
  const bezSzacunku = z("Inbox", "Przejrzeć komentarze", null, 0);
  const sesja = {
    projekt: montaz.projekt,
    zadanie: montaz.zadanie,
    szac_min: 120,
    sekundy_zamkniete: 45 * 60 + 41 * 60,
    odcinek_od_ms: stan === "praca" || stan === "jestes" ? teraz - 6 * MINUTA - 12_000 : null,
    pauza_od_ms: stan === "pauza" ? teraz - 3 * MINUTA - 4000 : null,
    powod_pauzy: stan === "pauza" ? ("reczna" as const) : null,
    bezczynna_od_ms: stan === "jestes" ? teraz - 16 * MINUTA : null,
  };
  const przeksztalcWybor = (przekroczony: boolean): Migawka["wybor"] => ({
    projekty: [
      { nazwa: "Film #1 · FIFA", zadania: [montaz, miniatura] },
      { nazwa: "Inbox", zadania: [mail, bezSzacunku] },
    ],
    suma_min: przekroczony ? 330 : 105,
    cel_min: 240,
    przekroczony,
    zaznaczonych: 2,
    mozna_wrocic: false,
  });
  const parametry = new URLSearchParams(location.search);
  return {
    stan,
    widoczna: true,
    strona,
    imie: "Dżiordżio",
    wybor: stan === "wybor_zadan" ? przeksztalcWybor(parametry.get("suma") === "duza") : null,
    lista:
      stan === "lista" || stan === "gratulacje"
        ? {
            zadania: [scenariusz, montaz, mail],
            zrobione: 1,
            razem: 3,
            pozostalo_min: 90,
          }
        : null,
    sesja: ["praca", "pauza", "jestes"].includes(stan) ? sesja : null,
    gratulacje:
      stan === "gratulacje"
        ? {
            zadanie: "Scenariusz: intro",
            czas_min: 47,
            szac_min: 60,
            czas_dnia_min: 132,
            plan_wykonany: parametry.get("plan") === "wykonany",
          }
        : null,
    koniec:
      stan === "koniec_dnia"
        ? {
            etap: (parametry.get("etap") as "reakcja" | "pozegnanie" | "spi" | "wczoraj" | null) ?? "reakcja",
            reakcja: (parametry.get("reakcja") as Reakcja | null) ?? "lekko_pochwalajaca",
            podsumowanie: {
              zrobione: 2,
              zaplanowane: 4,
              czas_dnia_min: 132,
              cel_min: 240,
              zadania: [
                { zadanie: "Scenariusz: intro", projekt: "Film #1 · FIFA", czas_min: 47, szac_min: 60, zrobione: true },
                { zadanie: "Montaż: plansze 1–5", projekt: "Film #1 · FIFA", czas_min: 45, szac_min: 120, zrobione: false },
                { zadanie: "Miniatura", projekt: "Film #1 · FIFA", czas_min: 40, szac_min: 30, zrobione: true },
              ],
              na_jutro: ["Montaż: plansze 1–5", "Odpisać na maila od montażysty"],
            },
          }
        : null,
    dzien: { minuty: 132, cel_min: 240 },
    blad_pliku: null,
    projekty_do_dopisania: ["Film #1 · FIFA", "Inbox"],
  };
}

const DYMKI: Partial<Record<NazwaStanu, Zdarzenie>> = {
  konfiguracja: { rodzaj: "dymek", klucz: "konfiguracja_imie", wstawki: {} },
  powitanie: { rodzaj: "dymek", klucz: "powitanie", wstawki: { imie: "Dżiordżio" } },
  wybor_zadan: { rodzaj: "dymek", klucz: "wybor", wstawki: {} },
  lista: { rodzaj: "dymek", klucz: "lista", wstawki: {} },
  praca: { rodzaj: "dymek", klucz: "start", wstawki: { zadanie: "Montaż: plansze 1–5" } },
  pauza: { rodzaj: "dymek", klucz: "pauza", wstawki: {} },
  jestes: { rodzaj: "dymek", klucz: "jestes", wstawki: {} },
  koniec_dnia: { rodzaj: "dymek", klucz: "koniec_pochwala", wstawki: { czas_dnia: "2 h 12 min" } },
  gratulacje: {
    rodzaj: "dymek",
    klucz: "zrobione",
    wstawki: { zadanie: "Scenariusz: intro", czas: "47 min", szac: "1 h", czas_dnia: "2 h 12 min" },
  },
};

const STANY: NazwaStanu[] = [
  "konfiguracja",
  "powitanie",
  "wybor_zadan",
  "lista",
  "praca",
  "pauza",
  "jestes",
  "gratulacje",
  "koniec_dnia",
];

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
    rozpocznij: async (projekt, zadanie) => console.info("rozpocznij", projekt, zadanie),
    przelaczWybor: async (projekt, zadanie) => console.info("przełącz wybór", projekt, zadanie),
    dodajZadanie: async (nazwa, szac, projekt) => console.info("dodaj zadanie", nazwa, szac, projekt),
    akcja: async (akcja: Akcja) => console.info("akcja", akcja),
    odpowiedzJestes: async (przerwa) => console.info("odpowiedź", przerwa),
    naMigawke: async () => undefined,
    naZdarzenie: async (obsluga) => {
      naZdarzenie = obsluga;
    },
    naKursor: async () => undefined,
    naAkcja: async () => undefined,
    menuKontekstowe: async () => console.info("menu pod prawym przyciskiem"),
    przeciagnij: () => undefined,
    zglosBlad: (tekst) => console.error(tekst),
    kreatorDane: async () => PODPOWIEDZ,
    kreatorZakoncz: async (ustawienia, utworz) => console.info("kreator zakończony", ustawienia, utworz),
    plikIstnieje: async (sciezka) => sciezka.endsWith("Zadania.md") && sciezka.includes("Istnieje"),
    wybierzPlik: async () => "D:\\Second Brain\\Zadania.md",
    wybierzFolder: async () => "D:\\Second Brain",
    fokus: async () => undefined,
    ustawieniaPobierz: async () => PODPOWIEDZ.ustawienia,
    ustawieniaZapisz: async (ustawienia) => console.info("zapisano", ustawienia),
    otworzUstawienia: async () => undefined,
    zamknijOkno: async () => console.info("zamknij okno"),
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
