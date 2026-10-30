// Widget: rysuje migawki z backendu, pokazuje dymki i efekty, zgłasza
// rozmiar treści, żeby okno było dopasowane do stanu (B6).

import "./formularz.css";
import "./style.css";

import { Dodawanie } from "./dodawanie";
import { Dymek, kwestia } from "./dymki";
import { KONFETTI_DLUGIE_MS, KONFETTI_MS, konfetti } from "./efekty";
import { type Geometria, Grafiki } from "./grafiki";
import { Kreator } from "./kreator";
import { type Akcja, type Most, mostTauri, wTauri } from "./most";
import { mostPodglad } from "./podglad";
import { Postac } from "./postac";
import type { Migawka, NazwaStanu, Zdarzenie } from "./typy";
import { aktualizuj, panel } from "./widok";

/** obok: panel z lewej, postać stoi z prawej; nad: postać siedzi na panelu. */
const UKLADY: Record<NazwaStanu, "obok" | "nad" | "sama"> = {
  konfiguracja: "obok",
  powitanie: "sama",
  wybor_zadan: "obok",
  lista: "obok",
  praca: "nad",
  pauza: "nad",
  jestes: "nad",
  gratulacje: "nad",
  koniec_dnia: "obok",
};

const Z_LICZNIKIEM = new Set<NazwaStanu>(["praca", "pauza", "jestes"]);
/** Stany, w których można dopisać zadanie (formularz zastępuje panel). */
const Z_DODAWANIEM = new Set<NazwaStanu>(["wybor_zadan", "lista", "praca", "pauza", "gratulacje"]);
const GLITCH_MS = 400;

function element<T extends HTMLElement = HTMLElement>(id: string): T {
  const znaleziony = document.getElementById(id);
  if (!znaleziony) throw new Error(`brak elementu #${id}`);
  return znaleziony as T;
}

/** Rozmiar postaci i linie stóp oraz siedziska z postac.json jako zmienne CSS. */
function ustawGeometrie(g: Geometria): void {
  const styl = document.documentElement.style;
  styl.setProperty("--postac-w", `${g.szer}px`);
  styl.setProperty("--postac-h", `${g.wys}px`);
  styl.setProperty("--pod-siedziskiem", `${g.podSiedziskiem}px`);
  styl.setProperty("--pod-stopami", `${g.podStopami}px`);
  styl.setProperty("--oddech-max", String(g.oddech));
  // Panele z przewijaniem mają do 60% wysokości ekranu (B6).
  styl.setProperty("--panel-max-h", `${Math.round(window.screen.availHeight * 0.6)}px`);
}

const most: Most = wTauri() ? mostTauri() : mostPodglad();

const opisBledu = (e: unknown) => (e instanceof Error ? `${e.message}\n${e.stack ?? ""}` : String(e));
window.addEventListener("error", (e) => most.zglosBlad(`${e.message} (${e.filename}:${e.lineno})`));
window.addEventListener("unhandledrejection", (e) => most.zglosBlad(opisBledu(e.reason)));

async function start(): Promise<void> {
  const scena = element("scena");
  const miejscePanelu = element("miejsce-panelu");
  const konfettiEl = element("konfetti");
  const grafiki = new Grafiki();
  ustawGeometrie(grafiki.geometria);
  const postac = new Postac(element("postac"), grafiki);

  let migawka: Migawka | null = null;
  let widoczna = true;
  let gotowy = false;
  let ostatniRozmiar = "";
  let zegarLicznika: number | undefined;
  let kreator: Kreator | null = null;
  let dodawanie: Dodawanie | null = null;
  let fokus = false;

  // Okno widgetu nie przejmuje fokusu, więc pola tekstowe (kreator) włączają go na czas wpisywania.
  const ustawFokus = (potrzebny: boolean) => {
    if (fokus === potrzebny) return;
    fokus = potrzebny;
    most.fokus(potrzebny).catch((e) => most.zglosBlad(opisBledu(e)));
  };

  const rozmiar = () => {
    const r = scena.getBoundingClientRect();
    return [Math.ceil(r.width), Math.ceil(r.height)] as const;
  };

  const zglosRozmiar = () => {
    if (!gotowy) return;
    const [szer, wys] = rozmiar();
    const klucz = `${szer}x${wys}`;
    if (klucz === ostatniRozmiar) return;
    ostatniRozmiar = klucz;
    void most.dopasuj(szer, wys);
  };

  // Rozmiar mierzymy synchronicznie: w ukrytym oknie requestAnimationFrame
  // nie działa, a okno pokazuje się dopiero po pierwszym pomiarze.
  const dymek = new Dymek(element("dymek"), zglosRozmiar);
  new ResizeObserver(zglosRozmiar).observe(scena);

  const glitch = (rodzaj: "wejscie" | "wyjscie") => {
    scena.classList.remove("glitch-wejscie", "glitch-wyjscie");
    void scena.offsetWidth;
    scena.classList.add(`glitch-${rodzaj}`);
    if (rodzaj === "wejscie") window.setTimeout(() => scena.classList.remove("glitch-wejscie"), GLITCH_MS);
  };

  // Licznik odświeża się raz na sekundę i tylko wtedy, gdy go widać.
  const ustawLicznik = () => {
    const m = migawka;
    const potrzebny = widoczna && m !== null && Z_LICZNIKIEM.has(m.stan);
    if (!potrzebny) {
      window.clearTimeout(zegarLicznika);
      zegarLicznika = undefined;
      return;
    }
    if (zegarLicznika !== undefined) return;
    const tik = () => {
      const teraz = Date.now();
      if (migawka) aktualizuj(miejscePanelu, migawka, teraz);
      const s = migawka?.sesja;
      const od = s?.odcinek_od_ms ?? s?.pauza_od_ms ?? teraz;
      zegarLicznika = window.setTimeout(tik, 1000 - ((teraz - od) % 1000) + 15);
    };
    tik();
  };

  const ustawWidocznosc = (nowa: boolean) => {
    if (nowa === widoczna) return;
    widoczna = nowa;
    postac.widocznosc(nowa);
    if (nowa) {
      document.body.classList.remove("uspiona");
      glitch("wejscie");
    } else {
      dymek.schowaj();
      glitch("wyjscie");
      // Schowane okno nie animuje niczego (B6, wydajność w grach).
      window.setTimeout(() => {
        if (!widoczna) document.body.classList.add("uspiona");
      }, GLITCH_MS);
    }
    ustawLicznik();
  };

  const rysuj = (m: Migawka) => {
    migawka = m;
    scena.dataset.stan = m.stan;
    scena.dataset.uklad = UKLADY[m.stan];
    scena.dataset.strona = m.strona;
    if (dodawanie && !Z_DODAWANIEM.has(m.stan)) dodawanie = null;
    if (m.stan === "konfiguracja") {
      // Kreator trzyma swój stan, więc nie przebudowujemy go przy każdej migawce.
      if (!kreator) {
        kreator = new Kreator(most, {
          poKroku: (krok) => {
            postac.krokKreatora(krok);
            if (krok === 1) {
              const tekst = kwestia("konfiguracja_plik", {});
              if (tekst) dymek.pokaz(tekst);
            }
          },
        });
        miejscePanelu.replaceChildren(kreator.korzen);
        kreator.start().catch((e) => most.zglosBlad(opisBledu(e)));
      }
    } else {
      kreator = null;
      // Formularz „Dodaj zadanie” trzyma wpisany tekst, więc migawki go nie przebudowują.
      if (!dodawanie) {
        const przewiniecie = miejscePanelu.querySelector(".przewijany")?.scrollTop ?? 0;
        const nowy = panel(m);
        miejscePanelu.replaceChildren(...(nowy ? [nowy] : []));
        const lista = miejscePanelu.querySelector(".przewijany");
        if (lista) lista.scrollTop = przewiniecie;
      }
    }
    ustawFokus(m.stan === "konfiguracja" || dodawanie !== null);
    aktualizuj(miejscePanelu, m, Date.now());
    postac.ustaw(m);
    ustawWidocznosc(m.widoczna);
    window.clearTimeout(zegarLicznika);
    zegarLicznika = undefined;
    ustawLicznik();
    zglosRozmiar();
  };

  const obsluzZdarzenie = (z: Zdarzenie) => {
    if (z.rodzaj === "dymek") {
      const tekst = kwestia(z.klucz, z.wstawki);
      if (tekst && widoczna) dymek.pokaz(tekst);
      return;
    }
    switch (z.nazwa) {
      case "konfetti":
        konfetti(konfettiEl, KONFETTI_MS);
        postac.podskocz();
        break;
      case "konfetti_dlugie":
        konfetti(konfettiEl, KONFETTI_DLUGIE_MS, 60);
        postac.podskocz();
        break;
      case "przeciaganie":
        postac.przeciagnijSie();
        break;
      case "glitch_wejscie":
        glitch("wejscie");
        break;
    }
  };

  const blad = (e: unknown) => most.zglosBlad(opisBledu(e));

  const otworzDodawanie = () => {
    const m = migawka;
    if (dodawanie || !m || !Z_DODAWANIEM.has(m.stan)) return;
    dodawanie = new Dodawanie(most, m.projekty_do_dopisania, { poZamknieciu: zamknijDodawanie });
    miejscePanelu.replaceChildren(dodawanie.korzen);
    ustawFokus(true);
    zglosRozmiar();
  };

  function zamknijDodawanie(): void {
    dodawanie = null;
    if (migawka) rysuj(migawka);
  }

  miejscePanelu.addEventListener("click", (e) => {
    const przycisk = (e.target as HTMLElement).closest<HTMLButtonElement>("button");
    if (!przycisk || przycisk.disabled) return;
    const { projekt, zadanie, akcja } = przycisk.dataset;
    if (projekt !== undefined && zadanie !== undefined) {
      most.rozpocznij(projekt, zadanie).catch(blad);
    } else if (akcja === "przerwa" || akcja === "pracowalem") {
      most.odpowiedzJestes(akcja === "przerwa").catch(blad);
    } else if (akcja === "dodaj") {
      otworzDodawanie();
    } else if (akcja === "otworz_ustawienia") {
      most.otworzUstawienia().catch(blad);
    } else if (akcja) {
      most.akcja(akcja as Akcja).catch(blad);
    }
  });

  // Pola wyboru zadań: stan zaznaczeń trzyma backend, więc tylko zgłaszamy przełączenie.
  miejscePanelu.addEventListener("change", (e) => {
    const pole = e.target as HTMLInputElement;
    if (pole.type === "checkbox" && pole.dataset.projekt !== undefined) {
      most.przelaczWybor(pole.dataset.projekt, pole.dataset.zadanie ?? "").catch(blad);
    }
  });

  // Prawy przycisk na postaci i panelu: menu z poleceniami (B6). Pola tekstowe zostawiamy
  // z systemowym menu (wytnij, wklej), żeby nie psuć wpisywania.
  document.addEventListener("contextmenu", (e) => {
    if ((e.target as HTMLElement).closest("input, textarea")) return;
    e.preventDefault();
    most.menuKontekstowe().catch(blad);
  });

  // Okno przeciąga się za postać; pozycja zapamiętuje się w backendzie.
  element("postac").addEventListener("mousedown", (e) => {
    if (e.button === 0 && e.detail === 1) most.przeciagnij();
  });

  await most.naMigawke(rysuj);
  await most.naZdarzenie(obsluzZdarzenie);
  await most.naKursor((blisko) => postac.kursor(blisko));
  await most.naAkcja((nazwa) => {
    if (nazwa === "dodaj_zadanie") otworzDodawanie();
  });
  rysuj(await most.migawka());
  await Promise.race([document.fonts.ready, new Promise((r) => window.setTimeout(r, 500))]);
  const [szer, wys] = rozmiar();
  ostatniRozmiar = `${szer}x${wys}`;
  gotowy = true;
  await most.gotowy(szer, wys);
}

start().catch((e) => most.zglosBlad(`start widgetu nie powiódł się: ${opisBledu(e)}`));
