import { describe, expect, it } from "vitest";

import { doZmianyFazyPrzerwy, doZmianyKonsolki, fazaPrzerwy, konsolkaDwa } from "./przerwa";

const SEK = 1000;
const MIN = 60 * SEK;

describe("fazaPrzerwy", () => {
  it("przeciąga się 5 s, potem gra na konsolce, od 15 min śpi", () => {
    expect(fazaPrzerwy(0)).toBe("rozciaganie");
    expect(fazaPrzerwy(4999)).toBe("rozciaganie");
    expect(fazaPrzerwy(5 * SEK)).toBe("konsola");
    expect(fazaPrzerwy(14 * MIN + 59 * SEK)).toBe("konsola");
    expect(fazaPrzerwy(15 * MIN)).toBe("drzemka");
    expect(fazaPrzerwy(3 * 60 * MIN)).toBe("drzemka");
  });

  it("po powrocie z blokady od razu daje pozę właściwą dla czasu przerwy", () => {
    // blokada o 10:00, odblokowanie o 10:20: przerwa trwa już 20 min
    expect(fazaPrzerwy(20 * MIN)).toBe("drzemka");
    // krótka blokada: 40 s
    expect(fazaPrzerwy(40 * SEK)).toBe("konsola");
  });

  it("podaje, kiedy zmieni się faza", () => {
    expect(doZmianyFazyPrzerwy(0)).toBe(5 * SEK);
    expect(doZmianyFazyPrzerwy(1200)).toBe(3800);
    expect(doZmianyFazyPrzerwy(5 * SEK)).toBe(15 * MIN - 5 * SEK);
    expect(doZmianyFazyPrzerwy(15 * MIN)).toBeNull();
  });
});

describe("konsolka", () => {
  it("sit_console_2 pojawia się tylko na 3 s co 2 min, pierwszy raz po 2 min", () => {
    expect(konsolkaDwa(0)).toBe(false);
    expect(konsolkaDwa(119 * SEK)).toBe(false);
    expect(konsolkaDwa(2 * MIN)).toBe(true);
    expect(konsolkaDwa(2 * MIN + 2999)).toBe(true);
    expect(konsolkaDwa(2 * MIN + 3 * SEK)).toBe(false);
    expect(konsolkaDwa(3 * MIN + 59 * SEK)).toBe(false);
    expect(konsolkaDwa(4 * MIN)).toBe(true);
    expect(konsolkaDwa(4 * MIN + 3 * SEK)).toBe(false);
  });

  it("przez godzinę druga klatka widoczna jest łącznie 29 razy po 3 s", () => {
    let widoczna = 0;
    for (let t = 0; t < 60 * MIN; t += 100) if (konsolkaDwa(t)) widoczna += 100;
    expect(widoczna).toBe(29 * 3 * SEK);
  });

  it("podaje, kiedy zmieni się poza", () => {
    expect(doZmianyKonsolki(0)).toBe(2 * MIN);
    expect(doZmianyKonsolki(2 * MIN - 500)).toBe(500);
    expect(doZmianyKonsolki(2 * MIN)).toBe(3 * SEK);
    expect(doZmianyKonsolki(2 * MIN + 1 * SEK)).toBe(2 * SEK);
    expect(doZmianyKonsolki(2 * MIN + 3 * SEK)).toBe(2 * MIN - 3 * SEK);
  });

  it("zegar zmian trafia dokładnie w granice cyklu", () => {
    let t = 0;
    const zmiany: number[] = [];
    while (t < 5 * MIN) {
      const za = doZmianyKonsolki(t);
      expect(za).toBeGreaterThan(0);
      t += za;
      zmiany.push(t);
    }
    expect(zmiany.slice(0, 5)).toEqual([2 * MIN, 2 * MIN + 3 * SEK, 4 * MIN, 4 * MIN + 3 * SEK, 6 * MIN]);
  });
});
