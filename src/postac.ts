// Postać: wybór pozy dla stanu i proste animacje z B7. Ciągłe ruchy
// (oddech, klatki machania) robi CSS; JS tylko przełącza klasy i obrazki
// w rzadkich chwilach: mrugnięcie co 3–7 s, zmiana pozy co 3–6 min.

import type { Grafiki, Poza } from "./grafiki";
import type { KoniecWidok, Migawka, NazwaStanu } from "./typy";

type Tryb = "brak" | "mrug" | "klatki";

interface Klatki {
  a: Poza;
  b?: Poza;
  tryb: Tryb;
  /** Długość cyklu dwóch klatek, np. machania. */
  cykl?: string;
}

type Wariant = "patrzy" | "konsola" | "kubek";

const MRUGNIECIE_MS = 130;
const WSKAZYWANIE_MS = 3000;
const PRZECIAGANIE_MS = 4000;
const DRZEMKA_PO_MS = 15 * 60 * 1000;
const ZANIK_MS = 75;

const losowo = (od: number, do_: number) => od + Math.random() * (do_ - od);

export class Postac {
  private readonly cialo: HTMLElement;
  private readonly obrazA: HTMLImageElement;
  private readonly obrazB: HTMLImageElement;
  private stan: NazwaStanu | null = null;
  private widoczna = true;
  private wskazuje = false;
  private przeciaga = false;
  private blisko = false;
  private drzemka = false;
  private wariant: Wariant = "patrzy";
  private pauzaOd: number | null = null;
  private krokKreatoraNr = 0;
  private numerZmiany = 0;
  private koniec: KoniecWidok | null = null;
  private zegarMrugania: number | undefined;
  private zegarWariantu: number | undefined;
  private zegarStanu: number | undefined;
  private zegarDrzemki: number | undefined;
  private obecne = "";

  constructor(
    private readonly korzen: HTMLElement,
    private readonly grafiki: Grafiki,
  ) {
    this.cialo = korzen.querySelector<HTMLElement>(".cialo")!;
    this.obrazA = korzen.querySelector<HTMLImageElement>(".klatka-a")!;
    this.obrazB = korzen.querySelector<HTMLImageElement>(".klatka-b")!;
    for (const adres of grafiki.wszystkie()) {
      new Image().src = adres;
    }
  }

  ustaw(m: Migawka): void {
    this.koniec = m.koniec;
    const pauzaOd = m.stan === "pauza" ? (m.sesja?.pauza_od_ms ?? null) : null;
    if (m.stan !== this.stan) {
      this.wejdzWStan(m.stan);
    }
    if (pauzaOd !== this.pauzaOd) {
      this.pauzaOd = pauzaOd;
      this.zaplanujDrzemke();
    }
    this.odswiez(false);
  }

  /** Kreator: przy pierwszym kroku macha, potem wskazuje plik. */
  krokKreatora(krok: number): void {
    this.krokKreatoraNr = krok;
    this.odswiez(true);
  }

  widocznosc(widoczna: boolean): void {
    if (this.widoczna === widoczna) return;
    this.widoczna = widoczna;
    if (widoczna) {
      this.zaplanujMrugniecie();
    } else {
      window.clearTimeout(this.zegarMrugania);
      this.cialo.classList.remove("mrug");
    }
  }

  kursor(blisko: boolean): void {
    if (this.blisko === blisko) return;
    this.blisko = blisko;
    this.odswiez(true);
  }

  /** Przypomnienie o przerwie: chwilę się przeciąga. */
  przeciagnijSie(): void {
    if (this.stan !== "praca") return;
    this.przeciaga = true;
    this.odswiez(true);
    window.setTimeout(() => {
      this.przeciaga = false;
      this.odswiez(true);
    }, PRZECIAGANIE_MS);
  }

  /** Gratulacje: dwa podskoki po 12 px (CSS). */
  podskocz(): void {
    this.korzen.classList.remove("podskoki");
    void this.korzen.offsetWidth;
    this.korzen.classList.add("podskoki");
  }

  private wejdzWStan(stan: NazwaStanu): void {
    const poprzedni = this.stan;
    this.stan = stan;
    window.clearTimeout(this.zegarStanu);
    window.clearTimeout(this.zegarWariantu);
    this.przeciaga = false;
    this.wskazuje = false;
    if (stan === "wybor_zadan" && this.grafiki.ma("point") && poprzedni !== "wybor_zadan") {
      this.wskazuje = true;
      this.zegarStanu = window.setTimeout(() => {
        this.wskazuje = false;
        this.odswiez(true);
      }, WSKAZYWANIE_MS);
    }
    if (stan === "praca") {
      if (poprzedni !== "jestes" && poprzedni !== "pauza") this.wariant = "patrzy";
      this.zaplanujZmianePozy();
    }
  }

  private dostepneWarianty(): Wariant[] {
    const warianty: Wariant[] = ["patrzy", "konsola"];
    if (this.grafiki.ma("sit_drink")) warianty.push("kubek");
    return warianty;
  }

  /** Losowo co 3–6 min jedna z póz siedzących (B7). */
  private zaplanujZmianePozy(): void {
    window.clearTimeout(this.zegarWariantu);
    this.zegarWariantu = window.setTimeout(
      () => {
        const inne = this.dostepneWarianty().filter((w) => w !== this.wariant);
        this.wariant = inne[Math.floor(Math.random() * inne.length)] ?? "patrzy";
        this.odswiez(true);
        this.zaplanujZmianePozy();
      },
      losowo(3, 6) * 60 * 1000,
    );
  }

  private zaplanujDrzemke(): void {
    window.clearTimeout(this.zegarDrzemki);
    this.drzemka = false;
    if (this.pauzaOd == null) return;
    const zostalo = this.pauzaOd + DRZEMKA_PO_MS - Date.now();
    if (zostalo <= 0) {
      this.drzemka = true;
      return;
    }
    this.zegarDrzemki = window.setTimeout(() => {
      this.drzemka = true;
      this.odswiez(true);
    }, zostalo);
  }

  private dwie(a: Poza, b: Poza, cykl: string): Klatki {
    return this.grafiki.ma(b) && this.grafiki.adres(a) !== this.grafiki.adres(b)
      ? { a, b, tryb: "klatki", cykl }
      : { a, tryb: "brak" };
  }

  /** Koniec dnia: reakcja z tabeli w B4 (pozy z postac.json), potem „bye”, a po schowaniu „pause_nap”. */
  private klatkiKonca(): Klatki {
    const k = this.koniec;
    if (!k) return { a: "stand_idle", b: "stand_idle_blink", tryb: "mrug" };
    if (k.etap === "pozegnanie") return { a: "bye", tryb: "brak" };
    if (k.etap === "spi") return { a: "pause_nap", tryb: "brak" };
    const [a, b] = (this.grafiki.reakcje[k.reakcja] ?? ["stand_idle"]) as Poza[];
    if (a && b) return this.dwie(a, b, "0.8s");
    return a === "stand_idle" ? { a, b: "stand_idle_blink", tryb: "mrug" } : { a: a ?? "stand_idle", tryb: "brak" };
  }

  private klatki(): Klatki {
    const g = this.grafiki;
    const dwie = (a: Poza, b: Poza, cykl: string): Klatki => this.dwie(a, b, cykl);
    const stoi: Klatki = { a: "stand_idle", b: "stand_idle_blink", tryb: "mrug" };
    switch (this.stan) {
      case "konfiguracja":
        return this.krokKreatoraNr === 0 ? dwie("wave_1", "wave_2", "0.7s") : { a: "point", tryb: "brak" };
      case "powitanie":
        return dwie("wave_1", "wave_2", "0.7s");
      case "wybor_zadan":
        return this.wskazuje ? { a: "point", tryb: "brak" } : stoi;
      case "lista":
        return stoi;
      case "praca":
        if (this.przeciaga) return { a: "pause_stretch", tryb: "brak" };
        if (this.blisko && g.ma("sit_watch_front")) return { a: "sit_watch_front", tryb: "brak" };
        if (this.wariant === "konsola") return dwie("sit_console_1", "sit_console_2", "1.6s");
        if (this.wariant === "kubek") return { a: "sit_drink", tryb: "brak" };
        return { a: "sit_watch", b: "sit_watch_blink", tryb: "mrug" };
      case "pauza":
        return { a: this.drzemka ? "pause_nap" : "pause_stretch", tryb: "brak" };
      case "jestes":
        return { a: "idle_check", tryb: "brak" };
      case "gratulacje":
        return dwie("cheer_1", "cheer_2", "0.8s");
      case "koniec_dnia":
        return this.klatkiKonca();
      default:
        return stoi;
    }
  }

  private odswiez(zanik: boolean): void {
    const k = this.klatki();
    const adresA = this.grafiki.adres(k.a);
    const adresB = k.b ? this.grafiki.adres(k.b) : "";
    const klucz = `${adresA}|${adresB}|${k.tryb}`;
    if (klucz === this.obecne) return;
    const pierwszy = this.obecne === "";
    this.obecne = klucz;
    // Każda zmiana pozy unieważnia wcześniejsze, jeszcze niezastosowane przejścia:
    // przestarzałe zanikanie nie może nadpisać nowszej pozy.
    const numer = ++this.numerZmiany;
    const zastosuj = () => {
      this.cialo.classList.remove("zanik");
      this.obrazA.src = adresA;
      this.obrazA.dataset.poza = k.a;
      if (adresB) this.obrazB.src = adresB;
      else this.obrazB.removeAttribute("src");
      this.obrazB.dataset.poza = k.b ?? "";
      this.cialo.classList.toggle("klatki", k.tryb === "klatki");
      this.cialo.style.setProperty("--klatka-czas", k.cykl ?? "0.7s");
      this.cialo.classList.remove("mrug");
      this.zaplanujMrugniecie();
    };
    // Zmiana pozy w Pracy przechodzi przez zanikanie 150 ms (B7).
    if (zanik && !pierwszy) {
      this.cialo.classList.add("zanik");
      window.setTimeout(() => {
        if (numer === this.numerZmiany) zastosuj();
        else this.cialo.classList.remove("zanik");
      }, ZANIK_MS);
    } else {
      zastosuj();
    }
  }

  /** Mrugnięcie: podmiana na *_blink na 130 ms, losowo co 3–7 s. */
  private zaplanujMrugniecie(): void {
    window.clearTimeout(this.zegarMrugania);
    if (!this.widoczna || !this.obecne.endsWith("|mrug")) return;
    this.zegarMrugania = window.setTimeout(() => {
      this.cialo.classList.add("mrug");
      window.setTimeout(() => {
        this.cialo.classList.remove("mrug");
        this.zaplanujMrugniecie();
      }, MRUGNIECIE_MS);
    }, losowo(3000, 7000));
  }
}
