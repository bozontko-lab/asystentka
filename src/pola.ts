// Wspólne kontrolki formularzy kreatora (widget) i okienka ustawień.

import { el } from "./dom";

/** Komunikat dla użytkownika z błędu z backendu (napis) albo wyjątku. */
export function opisBledu(e: unknown, domyslny = "Nieznany błąd."): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : domyslny;
}

/** Pole z etykietą nad wejściem i podpowiedzią pod nim. */
export function pole(id: string, etykieta: string, wejscie: HTMLElement, podpowiedz?: string): HTMLElement {
  return el(
    "div",
    { class: "pole" },
    el("label", { for: id }, etykieta),
    wejscie,
    podpowiedz && el("span", { class: "podpowiedz" }, podpowiedz),
  );
}

/** Pole wyboru z etykietą i opisem po prawej. */
export function przelacznik(id: string, etykieta: string, opis: string, wlaczony: boolean): [HTMLElement, HTMLInputElement] {
  const w = el("input", { id, type: "checkbox" });
  w.checked = wlaczony;
  return [
    el(
      "div",
      { class: "pole pole-przelacznik" },
      w,
      el("label", { for: id }, etykieta, el("span", { class: "podpowiedz" }, opis)),
    ),
    w,
  ];
}

export interface OpcjeSciezki {
  id: string;
  etykieta: string;
  /** Pole tekstowe ze ścieżką (kreator dodaje mu obsługę Enter). */
  wejscie: HTMLInputElement;
  przycisk: string;
  /** Okno wyboru pliku albo folderu; `null`, gdy anulowano. */
  wybor: (obecny: string) => Promise<string | null>;
  /** Wywoływane po wpisaniu i po wybraniu ścieżki. */
  poZmianie?: (wartosc: string) => void;
  /** Okno wyboru może się nie otworzyć; błąd trafia do logu, a formularz działa dalej. */
  naBlad: (e: unknown) => void;
  podpowiedz?: string;
}

/** Pole ścieżki z przyciskiem okna wyboru. */
export function poleSciezki(o: OpcjeSciezki): HTMLElement {
  if (o.poZmianie) {
    const poZmianie = o.poZmianie;
    o.wejscie.addEventListener("input", () => poZmianie(o.wejscie.value));
  }
  const b = el("button", { class: "przycisk", type: "button" }, o.przycisk);
  b.addEventListener("click", () => {
    void o
      .wybor(o.wejscie.value)
      .then((wybrane) => {
        if (!wybrane) return;
        o.wejscie.value = wybrane;
        o.poZmianie?.(wybrane);
      })
      .catch(o.naBlad);
  });
  return pole(o.id, o.etykieta, el("div", { class: "wiersz-sciezki" }, o.wejscie, b), o.podpowiedz);
}
