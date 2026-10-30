// Czasy sekwencji przerwy i konsolki (SPEC.md, B4, B7). Same obliczenia bez DOM,
// żeby dało się je sprawdzić testami.

/** Po wciśnięciu Pauzy postać przez 5 s się przeciąga, potem gra na konsolce. */
export const ROZCIAGANIE_MS = 5000;
/** Od 15 min przerwy postać zasypia. */
export const DRZEMKA_PO_MS = 15 * 60 * 1000;
/** `sit_console_2` pojawia się co 2 min... */
export const KONSOLA_CO_MS = 2 * 60 * 1000;
/** ...na 3 s. */
export const KONSOLA_DWA_MS = 3000;

export type FazaPrzerwy = "rozciaganie" | "konsola" | "drzemka";

/**
 * Poza przerwy wynika z czasu od jej początku, a nie z tego, co widać na ekranie,
 * więc po powrocie z blokady ekranu albo uśpienia od razu jest właściwa.
 */
export function fazaPrzerwy(odPoczatkuMs: number): FazaPrzerwy {
  if (odPoczatkuMs < ROZCIAGANIE_MS) return "rozciaganie";
  if (odPoczatkuMs < DRZEMKA_PO_MS) return "konsola";
  return "drzemka";
}

/** Za ile ms zmieni się faza przerwy; `null`, gdy postać już śpi. */
export function doZmianyFazyPrzerwy(odPoczatkuMs: number): number | null {
  if (odPoczatkuMs < ROZCIAGANIE_MS) return ROZCIAGANIE_MS - odPoczatkuMs;
  if (odPoczatkuMs < DRZEMKA_PO_MS) return DRZEMKA_PO_MS - odPoczatkuMs;
  return null;
}

/**
 * Czy na konsolce widać `sit_console_2`: na 3 s co 2 min, pierwszy raz po 2 min od
 * początku konsolki. Poza tym stoi `sit_console_1`, z samym oddechem.
 */
export function konsolkaDwa(odPoczatkuKonsolkiMs: number): boolean {
  if (odPoczatkuKonsolkiMs < KONSOLA_CO_MS) return false;
  return odPoczatkuKonsolkiMs % KONSOLA_CO_MS < KONSOLA_DWA_MS;
}

/** Za ile ms zmieni się poza konsolki. */
export function doZmianyKonsolki(odPoczatkuKonsolkiMs: number): number {
  if (odPoczatkuKonsolkiMs < KONSOLA_CO_MS) return KONSOLA_CO_MS - odPoczatkuKonsolkiMs;
  const wCyklu = odPoczatkuKonsolkiMs % KONSOLA_CO_MS;
  return wCyklu < KONSOLA_DWA_MS ? KONSOLA_DWA_MS - wCyklu : KONSOLA_CO_MS - wCyklu;
}
