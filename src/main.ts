// Widget: rysuje migawki z backendu, pokazuje dymki i efekty, zgłasza
// rozmiar treści, żeby okno było dopasowane do stanu (B6).

import "./style.css";

import { Dymek, kwestia } from "./dymki";
import { KONFETTI_DLUGIE_MS, KONFETTI_MS, konfetti } from "./efekty";
import { type Geometria, Grafiki } from "./grafiki";
import { type Akcja, type Most, mostTauri, wTauri } from "./most";
import { mostPodglad } from "./podglad";
import { Postac } from "./postac";
import type { Migawka, NazwaStanu, Zdarzenie } from "./typy";
import { aktualizuj, panel } from "./widok";

/** obok: panel z lewej, postać stoi z prawej; nad: postać siedzi na panelu. */
const UKLADY: Record<NazwaStanu, "obok" | "nad" | "sama"> = {
  powitanie: "sama",
  lista: "obok",
  czeka_na_plan: "obok",
  koniec_dnia: "obok",
  praca: "nad",
  pauza: "nad",
  jestes: "nad",
  gratulacje: "nad",
};

const Z_LICZNIKIEM = new Set<NazwaStanu>(["praca", "pauza", "jestes"]);
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
    const nowy = panel(m);
    miejscePanelu.replaceChildren(...(nowy ? [nowy] : []));
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

  miejscePanelu.addEventListener("click", (e) => {
    const przycisk = (e.target as HTMLElement).closest<HTMLButtonElement>("button");
    if (!przycisk || przycisk.disabled) return;
    const { id, akcja } = przycisk.dataset;
    if (id) {
      most.rozpocznij(id).catch(blad);
    } else if (akcja === "przerwa" || akcja === "pracowalem") {
      most.odpowiedzJestes(akcja === "przerwa").catch(blad);
    } else if (akcja) {
      most.akcja(akcja as Akcja).catch(blad);
    }
  });

  // Okno przeciąga się za postać; pozycja zapamiętuje się w backendzie.
  element("postac").addEventListener("mousedown", (e) => {
    if (e.button === 0 && e.detail === 1) most.przeciagnij();
  });

  await most.naMigawke(rysuj);
  await most.naZdarzenie(obsluzZdarzenie);
  await most.naKursor((blisko) => postac.kursor(blisko));
  rysuj(await most.migawka());
  await Promise.race([document.fonts.ready, new Promise((r) => window.setTimeout(r, 500))]);
  const [szer, wys] = rozmiar();
  ostatniRozmiar = `${szer}x${wys}`;
  gotowy = true;
  await most.gotowy(szer, wys);
}

start().catch((e) => most.zglosBlad(`start widgetu nie powiódł się: ${opisBledu(e)}`));
