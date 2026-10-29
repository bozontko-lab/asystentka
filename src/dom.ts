// Małe pomocniki do budowania DOM bez frameworka.

export type Dziecko = Node | string | null | undefined | false;
export type Atrybuty = Record<string, string | boolean | undefined>;

export function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  atrybuty: Atrybuty = {},
  ...dzieci: (Dziecko | Dziecko[])[]
): HTMLElementTagNameMap[K] {
  const element = document.createElement(tag);
  for (const [nazwa, wartosc] of Object.entries(atrybuty)) {
    if (wartosc === true) element.setAttribute(nazwa, "");
    else if (typeof wartosc === "string") element.setAttribute(nazwa, wartosc);
  }
  for (const dziecko of dzieci.flat()) {
    if (dziecko === null || dziecko === undefined || dziecko === false) continue;
    element.append(dziecko);
  }
  return element;
}
