// Skrót pokazujący i chowający widget (SPEC.md, B6): w ustawieniach pole nagrywa
// naciśniętą kombinację, od razu sprawdza w backendzie, czy da się ją zarejestrować,
// i odrzuca Ctrl+Alt z literą (na polskiej klawiaturze to AltGr).

import { el } from "./dom";
import type { Most } from "./most";

/** Pola zdarzenia klawiatury, których potrzebuje nagrywanie (da się je podać w teście). */
export interface ZdarzenieKlawisza {
  code: string;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  metaKey: boolean;
}

const MODYFIKATORY = new Set([
  "ControlLeft",
  "ControlRight",
  "AltLeft",
  "AltRight",
  "ShiftLeft",
  "ShiftRight",
  "MetaLeft",
  "MetaRight",
  "OSLeft",
  "OSRight",
]);

/** Klawisz główny jak w parserze skrótów: litery i cyfry bez przedrostka, reszta jak `KeyboardEvent.code`. */
export function nazwaKlawisza(code: string): string {
  const znak = /^(?:Key|Digit)(.)$/.exec(code);
  return znak?.[1] ?? code;
}

/**
 * Kombinacja z naciśnięcia klawisza, np. `Ctrl+Alt+Shift+Space`. Zwraca `null`, gdy
 * naciśnięto sam modyfikator: czekamy wtedy na klawisz główny.
 */
export function kombinacjaZeZdarzenia(e: ZdarzenieKlawisza): string | null {
  if (MODYFIKATORY.has(e.code)) return null;
  const czesci: string[] = [];
  if (e.ctrlKey) czesci.push("Ctrl");
  if (e.altKey) czesci.push("Alt");
  if (e.shiftKey) czesci.push("Shift");
  if (e.metaKey) czesci.push("Super");
  czesci.push(nazwaKlawisza(e.code));
  return czesci.join("+");
}

const NAZWY: Record<string, string> = {
  Space: "Spacja",
  Super: "Win",
  Escape: "Esc",
  ArrowUp: "↑",
  ArrowDown: "↓",
  ArrowLeft: "←",
  ArrowRight: "→",
};

/** Skrót do pokazania człowiekowi: `Ctrl + Alt + Shift + Spacja`. */
export function skrotDlaCzlowieka(skrot: string): string {
  return skrot
    .split("+")
    .map((czesc) => NAZWY[czesc.trim()] ?? czesc.trim())
    .join(" + ");
}

const PODPOWIEDZ = "Kliknij pole i naciśnij nową kombinację. Esc anuluje.";

type PrzyciskiMostu = Pick<Most, "skrotSprawdz" | "skrotWstrzymaj">;

/** Pole skrótu w ustawieniach: nagrywa kombinację i od razu ją sprawdza. */
export class PoleSkrotu {
  readonly korzen: HTMLElement;
  private wartosc: string;
  private nagrywa = false;
  private sprawdza = false;
  private readonly pole: HTMLElement;
  private readonly wynik: HTMLElement;
  private readonly poNacisnieciu = (e: KeyboardEvent) => this.klawisz(e);

  constructor(
    private readonly most: PrzyciskiMostu,
    poczatkowy: string,
  ) {
    this.wartosc = poczatkowy;
    this.pole = el("div", { id: "skrot", class: "pole-skrotu", role: "button", tabindex: "0" });
    this.wynik = el("span", { class: "podpowiedz", role: "status" });
    this.korzen = el(
      "div",
      { class: "pole" },
      el("label", { for: "skrot" }, "Skrót pokazujący i chowający widget"),
      this.pole,
      this.wynik,
    );
    this.pole.addEventListener("click", () => this.start());
    this.pole.addEventListener("keydown", (e) => {
      if (!this.nagrywa && (e.key === "Enter" || e.key === " ")) {
        e.preventDefault();
        this.start();
      }
    });
    this.pole.addEventListener("blur", () => this.zakoncz(PODPOWIEDZ));
    window.addEventListener("pagehide", () => this.zakoncz(""));
    this.odswiez(PODPOWIEDZ);
  }

  /** Skrót, który ma trafić do ustawień (zapis kanoniczny po ostatnim udanym nagraniu). */
  get skrot(): string {
    return this.wartosc;
  }

  private odswiez(komunikat: string, blad = false): void {
    this.pole.textContent = this.nagrywa ? "Naciśnij kombinację klawiszy…" : skrotDlaCzlowieka(this.wartosc);
    this.pole.classList.toggle("nagrywa", this.nagrywa);
    this.pole.setAttribute("aria-invalid", blad ? "true" : "false");
    this.wynik.textContent = komunikat;
    this.wynik.classList.toggle("blad-skrotu", blad);
    this.wynik.setAttribute("role", blad ? "alert" : "status");
  }

  private start(): void {
    if (this.nagrywa) return;
    this.nagrywa = true;
    this.pole.focus();
    // Własny skrót widgetu nie może przechwycić naciśnięcia, które pole ma nagrać.
    void this.most.skrotWstrzymaj(true);
    document.addEventListener("keydown", this.poNacisnieciu, true);
    this.odswiez("Naciśnij kombinację z Ctrl, Alt, Shift albo Win. Esc anuluje.");
  }

  private zakoncz(komunikat: string, blad = false): void {
    if (!this.nagrywa) return;
    this.nagrywa = false;
    document.removeEventListener("keydown", this.poNacisnieciu, true);
    void this.most.skrotWstrzymaj(false);
    this.odswiez(komunikat, blad);
  }

  private klawisz(e: KeyboardEvent): void {
    e.preventDefault();
    e.stopPropagation();
    if (e.repeat || this.sprawdza) return;
    const bezModyfikatorow = !(e.ctrlKey || e.altKey || e.shiftKey || e.metaKey);
    if (e.code === "Escape" && bezModyfikatorow) return this.zakoncz(PODPOWIEDZ);
    const kombinacja = kombinacjaZeZdarzenia(e);
    if (kombinacja === null) return;
    this.sprawdza = true;
    this.most
      .skrotSprawdz(kombinacja)
      .then((kanoniczny) => {
        this.wartosc = kanoniczny;
        this.zakoncz("✓ Skrót jest dozwolony i wolny. Zapisz, żeby go używać.");
      })
      .catch((blad: unknown) => {
        // Zły skrót zostawia pole w nagrywaniu: od razu można nacisnąć inny.
        this.odswiez(typeof blad === "string" ? blad : "Nie mogę sprawdzić skrótu.", true);
      })
      .finally(() => {
        this.sprawdza = false;
      });
  }
}
