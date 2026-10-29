// Kreator pierwszego uruchomienia (SPEC.md, B4, stan „Konfiguracja”): pięć
// kroków w panelu widgetu. Wypełnia się tym, co już jest w ustawieniach.

import { type Dziecko, el } from "./dom";
import type { Most } from "./most";
import type { PodpowiedzKreatora, Ustawienia } from "./typy";

const KROKI = 5;
const NAZWA_PLIKU = "Zadania.md";

type Tryb = "nowy" | "wlasny";

export function separator(sciezka: string): string {
  return sciezka.includes("\\") ? "\\" : "/";
}

export function polacz(folder: string, nazwa: string): string {
  const f = folder.trim().replace(/[\\/]+$/, "");
  return f === "" ? nazwa : `${f}${separator(folder)}${nazwa}`;
}

export function folderNadrzedny(sciezka: string): string {
  const s = sciezka.trim().replace(/[\\/]+$/, "");
  const i = Math.max(s.lastIndexOf("\\"), s.lastIndexOf("/"));
  return i > 0 ? s.slice(0, i) : "";
}

export function nazwaFolderu(sciezka: string): string {
  const s = sciezka.trim().replace(/[\\/]+$/, "");
  return s.slice(Math.max(s.lastIndexOf("\\"), s.lastIndexOf("/")) + 1) || s;
}

/** Godziny z pola (przecinek albo kropka) na minuty; `null`, gdy to nie liczba z zakresu 0,25–24 h. */
export function godzinyNaMinuty(tekst: string): number | null {
  const h = Number.parseFloat(tekst.trim().replace(",", "."));
  if (!Number.isFinite(h) || h < 0.25 || h > 24) return null;
  return Math.round(h * 60);
}

export function minutyNaGodziny(minuty: number): string {
  return String(Math.round((minuty / 60) * 100) / 100).replace(".", ",");
}

export interface OpcjeKreatora {
  /** Zmiana kroku: postać i dymek reagują na nowy krok. */
  poKroku(krok: number): void;
}

export class Kreator {
  readonly korzen = el("section", { class: "panel panel-kreator" });
  private krok = 0;
  private dane: PodpowiedzKreatora | null = null;
  private u!: Ustawienia;
  private tryb: Tryb = "nowy";
  private folderNowego = "";
  private plikWlasny = "";
  private dziennikRecznie = false;
  private blad = "";
  private zajety = false;

  constructor(
    private readonly most: Most,
    private readonly opcje: OpcjeKreatora,
  ) {}

  async start(): Promise<void> {
    this.dane = await this.most.kreatorDane();
    this.u = { ...this.dane.ustawienia };
    this.tryb = this.dane.plik_istnieje ? "wlasny" : "nowy";
    this.plikWlasny = this.u.plik_zadan;
    this.folderNowego = folderNadrzedny(this.u.plik_zadan);
    // Folder dziennika z wersji 1 to wybór użytkownika, a podpowiedź obok pliku zadań nie.
    this.dziennikRecznie = this.u.folder_dziennika !== "" && this.u.folder_dziennika !== this.domyslnyDziennik();
    this.render();
    this.opcje.poKroku(this.krok);
  }

  private plikZadan(): string {
    return this.tryb === "nowy" ? polacz(this.folderNowego, NAZWA_PLIKU) : this.plikWlasny.trim();
  }

  private domyslnyDziennik(): string {
    const folder = this.tryb === "nowy" ? this.folderNowego : folderNadrzedny(this.plikWlasny);
    return folder === "" ? "" : polacz(folder, "Dziennik pracy");
  }

  private render(): void {
    const k = this.krok;
    const tresc = [this.krokImie, this.krokPlik, this.krokDziennik, this.krokCel, this.krokAutostart][k];
    const czesci: (Dziecko | Dziecko[])[] = [
      el("div", { class: "krok" }, `Krok ${k + 1} z ${KROKI}`),
      tresc ? tresc.call(this) : null,
      this.blad !== "" && el("div", { class: "blad-formularza", role: "alert" }, this.blad),
      el(
        "div",
        { class: "przyciski" },
        k > 0 && el("button", { class: "przycisk cichy", type: "button", "data-kreator": "wstecz" }, "Wstecz"),
        el(
          "button",
          { class: "przycisk glowny", type: "button", "data-kreator": "dalej", disabled: this.zajety },
          k === KROKI - 1 ? "Gotowe" : "Dalej",
        ),
      ),
    ];
    this.korzen.replaceChildren(
      ...czesci.flat().filter((c): c is Node | string => c !== null && c !== undefined && c !== false),
    );
    this.korzen.querySelector('[data-kreator="wstecz"]')?.addEventListener("click", () => this.idz(-1));
    this.korzen.querySelector('[data-kreator="dalej"]')?.addEventListener("click", () => void this.dalej());
    this.korzen.querySelector<HTMLInputElement>("input[type=text], input[type=number]")?.focus();
  }

  private wejscie(id: string, wartosc: string, opcje: { typ?: string; krok?: string } = {}): HTMLInputElement {
    const w = el("input", { id, type: opcje.typ ?? "text", value: wartosc, spellcheck: "false" });
    if (opcje.krok) w.step = opcje.krok;
    w.value = wartosc;
    w.addEventListener("keydown", (e) => {
      if (e.key === "Enter") void this.dalej();
    });
    return w;
  }

  private krokImie(): Node[] {
    const w = this.wejscie("imie", this.u.imie);
    w.addEventListener("input", () => (this.u.imie = w.value));
    return [
      el("p", { class: "pytanie" }, "Jak mam się do Ciebie zwracać?"),
      el(
        "div",
        { class: "pole" },
        el("label", { for: "imie" }, "Forma w dymkach"),
        w,
        el("span", { class: "podpowiedz" }, "Wpisz ją tak, jak ma brzmieć, np. w wołaczu. Wstawię ją bez zmian."),
      ),
    ];
  }

  private przyciskWyboru(tekst: string, akcja: () => Promise<void>): HTMLButtonElement {
    const b = el("button", { class: "przycisk", type: "button" }, tekst);
    b.addEventListener("click", () => void akcja());
    return b;
  }

  private vaulty(poWyborze: (folder: string) => void): Node | null {
    const v = this.dane?.vaulty ?? [];
    if (v.length === 0) return null;
    return el(
      "div",
      { class: "vaulty", "aria-label": "Znalezione vaulty Obsidiana" },
      v.map((folder) => {
        const b = el("button", { class: "mala", type: "button", title: folder }, nazwaFolderu(folder));
        b.addEventListener("click", () => poWyborze(folder));
        return b;
      }),
    );
  }

  private krokPlik(): Node[] {
    const tryby = el(
      "div",
      { class: "wybor-trybu", role: "radiogroup" },
      ...(["nowy", "wlasny"] as const).map((t) => {
        const r = el("input", { type: "radio", name: "tryb", value: t });
        r.checked = this.tryb === t;
        r.addEventListener("change", () => {
          this.tryb = t;
          this.blad = "";
          this.render();
        });
        return el("label", {}, r, t === "nowy" ? "Załóż nowy plik z przykładami" : "Mam własny plik zadań");
      }),
    );
    const pole =
      this.tryb === "nowy"
        ? this.poleFolderuNowego()
        : this.poleSciezki("Plik zadań", this.plikWlasny, "Wskaż plik…", async (obecny) => {
            return this.most.wybierzPlik(obecny);
          }, (wartosc) => (this.plikWlasny = wartosc));
    return [el("p", { class: "pytanie" }, "Gdzie trzymasz zadania?"), tryby, pole];
  }

  private poleFolderuNowego(): Node {
    const wej = this.wejscie("folder-nowego", this.folderNowego);
    wej.addEventListener("input", () => (this.folderNowego = wej.value));
    const wybierz = this.przyciskWyboru("Wybierz folder…", async () => {
      const f = await this.most.wybierzFolder(wej.value, "Folder na plik Zadania.md");
      if (f) {
        this.folderNowego = f;
        wej.value = f;
      }
    });
    return el(
      "div",
      { class: "pole" },
      el("label", { for: "folder-nowego" }, `Folder dla pliku ${NAZWA_PLIKU}`),
      el("div", { class: "wiersz-sciezki" }, wej, wybierz),
      this.vaulty((f) => {
        this.folderNowego = f;
        wej.value = f;
      }),
      el("span", { class: "podpowiedz" }, "Najlepiej główny folder vaulta Obsidiana. Plik otworzę w Obsidianie, a istniejącego nie nadpiszę."),
    );
  }

  private poleSciezki(
    etykieta: string,
    wartosc: string,
    przycisk: string,
    wybor: (obecny: string) => Promise<string | null>,
    poZmianie: (wartosc: string) => void,
    podpowiedz?: string,
  ): Node {
    const id = `pole-${etykieta.replace(/\W+/g, "-")}`;
    const wej = this.wejscie(id, wartosc);
    wej.addEventListener("input", () => poZmianie(wej.value));
    const b = this.przyciskWyboru(przycisk, async () => {
      const wybrane = await wybor(wej.value);
      if (wybrane) {
        wej.value = wybrane;
        poZmianie(wybrane);
      }
    });
    return el(
      "div",
      { class: "pole" },
      el("label", { for: id }, etykieta),
      el("div", { class: "wiersz-sciezki" }, wej, b),
      podpowiedz && el("span", { class: "podpowiedz" }, podpowiedz),
    );
  }

  private krokDziennik(): Node[] {
    if (!this.dziennikRecznie || this.u.folder_dziennika === "") {
      this.u.folder_dziennika = this.domyslnyDziennik() || this.u.folder_dziennika;
    }
    return [
      el("p", { class: "pytanie" }, "Gdzie zapisywać dziennik pracy?"),
      this.poleSciezki(
        "Folder dziennika",
        this.u.folder_dziennika,
        "Wybierz folder…",
        (obecny) => this.most.wybierzFolder(obecny, "Folder dziennika pracy"),
        (wartosc) => {
          this.u.folder_dziennika = wartosc;
          this.dziennikRecznie = true;
        },
        "Każdy dzień to osobna notatka RRRR-MM-DD.md. Brakujący folder założę.",
      ),
    ];
  }

  private krokCel(): Node[] {
    const w = this.wejscie("cel", minutyNaGodziny(this.u.cel_dzienny_min), { typ: "text" });
    w.inputMode = "decimal";
    w.addEventListener("input", () => {
      const min = godzinyNaMinuty(w.value);
      w.setAttribute("aria-invalid", String(min === null));
      if (min !== null) this.u.cel_dzienny_min = min;
    });
    return [
      el("p", { class: "pytanie" }, "Ile godzin pracy dziennie chcesz sobie wyznaczyć?"),
      el(
        "div",
        { class: "pole" },
        el("label", { for: "cel" }, "Dzienny cel (godziny)"),
        w,
        el("span", { class: "podpowiedz" }, "Do niego porównuję sumę zadań i przepracowany czas. Zmienisz go w ustawieniach."),
      ),
    ];
  }

  private krokAutostart(): Node[] {
    const p = el("input", { id: "autostart", type: "checkbox" });
    p.checked = this.u.autostart;
    p.addEventListener("change", () => (this.u.autostart = p.checked));
    return [
      el("p", { class: "pytanie" }, "Uruchamiać mnie razem z Windows?"),
      el(
        "div",
        { class: "pole pole-przelacznik" },
        p,
        el(
          "label",
          { for: "autostart" },
          "Tak, startuj po zalogowaniu",
          el("span", { class: "podpowiedz" }, "Co rano przywitam Cię i zapytam o plan dnia."),
        ),
      ),
      el(
        "p",
        { class: "podpowiedz podsumowanie-sciezek" },
        `Plik zadań: ${this.plikZadan()}`,
        el("br"),
        `Dziennik: ${this.u.folder_dziennika}`,
      ),
    ];
  }

  private idz(o: number): void {
    this.blad = "";
    this.krok = Math.min(KROKI - 1, Math.max(0, this.krok + o));
    this.render();
    this.opcje.poKroku(this.krok);
  }

  private async sprawdzKrok(): Promise<string> {
    switch (this.krok) {
      case 0:
        return this.u.imie.trim() === "" ? "Wpisz, jak mam się do Ciebie zwracać." : "";
      case 1: {
        if (this.tryb === "nowy") {
          if (this.folderNowego.trim() === "") return "Wskaż folder dla pliku zadań.";
          const istniejacy = polacz(this.folderNowego, NAZWA_PLIKU);
          if (await this.most.plikIstnieje(istniejacy)) {
            // W tym folderze już jest taki plik: nie nadpisuję, tylko go używam.
            this.tryb = "wlasny";
            this.plikWlasny = istniejacy;
          }
          return "";
        }
        if (this.plikWlasny.trim() === "") return "Wskaż plik zadań.";
        return (await this.most.plikIstnieje(this.plikWlasny)) ? "" : "Nie widzę tego pliku. Sprawdź ścieżkę.";
      }
      case 2:
        return this.u.folder_dziennika.trim() === "" ? "Wskaż folder dziennika." : "";
      case 3:
        return godzinyNaMinuty(minutyNaGodziny(this.u.cel_dzienny_min)) === null
          ? "Cel dzienny to liczba godzin od 0,25 do 24."
          : "";
      default:
        return "";
    }
  }

  private async dalej(): Promise<void> {
    if (this.zajety) return;
    const blad = await this.sprawdzKrok();
    if (blad !== "") {
      this.blad = blad;
      this.render();
      return;
    }
    if (this.krok < KROKI - 1) {
      this.idz(1);
      return;
    }
    this.zajety = true;
    this.blad = "";
    this.render();
    const utworz = this.tryb === "nowy";
    const ustawienia: Ustawienia = { ...this.u, plik_zadan: this.plikZadan() };
    try {
      await this.most.kreatorZakoncz(ustawienia, utworz);
    } catch (e) {
      this.blad = typeof e === "string" ? e : e instanceof Error ? e.message : "Nie udało się zapisać ustawień.";
      this.zajety = false;
      this.render();
    }
  }
}
