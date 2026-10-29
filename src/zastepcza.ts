// Postać zastępcza (B9, krok 1): prosta chibi w SVG z palety z A2, w tych
// samych pozach co grafiki z części A. Znika, gdy w src/assets/postac/ pojawią
// się prawdziwe PNG.

import type { Poza } from "./grafiki";

const K = {
  lawenda: "#B9A6FF",
  lawendaCien: "#9A86EE",
  brzoskwinia: "#FFB59A",
  zolty: "#FFD84D",
  zoltyCien: "#EBBE2C",
  cyjan: "#2EDBE8",
  granat: "#1E2140",
  sluchawki: "#2C3163",
  skora: "#FFE4D6",
  rumieniec: "#FFB3B8",
  bursztyn: "#F0A23A",
  zrenica: "#4A2A0E",
  usta: "#B8475C",
  jezyk: "#FF8FA3",
  bialy: "#FFFFFF",
};

type Rece =
  | "luzno"
  | "macha"
  | "macha2"
  | "wskazuje"
  | "w_gore"
  | "nad_oczami"
  | "przeciaga"
  | "kolana"
  | "konsola"
  | "kubek"
  | "ekran"
  | "v";
type Oczy = "otwarte" | "w_gore_lewo" | "w_dol" | "w_bok" | "zamkniete" | "radosne" | "senne" | "oczko";
type Usta = "usmiech" | "szeroki" | "otwarte" | "jezyk" | "spokojne" | "brak";

interface Opis {
  siedzi?: boolean;
  rece: Rece;
  oczy: Oczy;
  usta: Usta;
  przechyl?: number;
  pochylenie?: number;
  zzz?: boolean;
}

const OPISY: Record<Poza, Opis> = {
  stand_idle: { rece: "luzno", oczy: "otwarte", usta: "usmiech" },
  stand_idle_blink: { rece: "luzno", oczy: "zamkniete", usta: "usmiech" },
  wave_1: { rece: "macha", oczy: "radosne", usta: "szeroki" },
  wave_2: { rece: "macha2", oczy: "radosne", usta: "szeroki" },
  point: { rece: "wskazuje", oczy: "otwarte", usta: "otwarte" },
  sit_watch: { siedzi: true, rece: "kolana", oczy: "w_gore_lewo", usta: "usmiech" },
  sit_watch_blink: { siedzi: true, rece: "kolana", oczy: "zamkniete", usta: "usmiech" },
  sit_watch_front: { siedzi: true, rece: "kolana", oczy: "otwarte", usta: "szeroki" },
  sit_console_1: { siedzi: true, rece: "konsola", oczy: "w_dol", usta: "spokojne" },
  sit_console_2: { siedzi: true, rece: "konsola", oczy: "radosne", usta: "jezyk", pochylenie: 5 },
  sit_drink: { siedzi: true, rece: "kubek", oczy: "senne", usta: "brak" },
  pause_stretch: { siedzi: true, rece: "przeciaga", oczy: "zamkniete", usta: "otwarte" },
  pause_nap: { siedzi: true, rece: "kolana", oczy: "zamkniete", usta: "spokojne", przechyl: 14, zzz: true },
  idle_check: { rece: "nad_oczami", oczy: "otwarte", usta: "otwarte", pochylenie: 7 },
  cheer_1: { rece: "w_gore", oczy: "radosne", usta: "szeroki" },
  cheer_2: { rece: "v", oczy: "oczko", usta: "szeroki" },
  bye: { rece: "macha", oczy: "senne", usta: "usmiech" },
  loading: { rece: "ekran", oczy: "w_bok", usta: "spokojne" },
  concerned: { rece: "luzno", oczy: "w_dol", usta: "spokojne", pochylenie: 3 },
};

type Punkt = [number, number];
type Ramie = [Punkt, Punkt, Punkt];

const LUZNO_L: Ramie = [[74, 114], [68, 136], [66, 154]];
const LUZNO_P: Ramie = [[126, 114], [132, 136], [134, 154]];

const RAMIONA: Record<Rece, [Ramie, Ramie]> = {
  luzno: [LUZNO_L, LUZNO_P],
  macha: [LUZNO_L, [[126, 114], [146, 100], [152, 70]]],
  macha2: [LUZNO_L, [[126, 114], [150, 104], [166, 80]]],
  wskazuje: [[[74, 114], [54, 118], [34, 112]], LUZNO_P],
  w_gore: [[[74, 114], [56, 84], [48, 38]], [[126, 114], [144, 84], [152, 38]]],
  nad_oczami: [LUZNO_L, [[126, 114], [140, 84], [110, 50]]],
  przeciaga: [[[74, 114], [62, 72], [88, 12]], [[126, 114], [138, 72], [112, 12]]],
  kolana: [[[74, 114], [64, 138], [70, 154]], [[126, 114], [136, 138], [130, 154]]],
  konsola: [[[74, 114], [74, 138], [88, 146]], [[126, 114], [126, 138], [112, 146]]],
  kubek: [[[74, 114], [80, 136], [93, 130]], [[126, 114], [120, 136], [107, 130]]],
  ekran: [LUZNO_L, [[126, 114], [146, 112], [156, 96]]],
  v: [LUZNO_L, [[126, 114], [144, 104], [146, 78]]],
};

const obrys = `stroke="${K.granat}" stroke-width="2.2" stroke-linejoin="round" stroke-linecap="round"`;

function ramie([bark, lokiec, dlon]: Ramie): string {
  const d = `M${bark[0]},${bark[1]} L${lokiec[0]},${lokiec[1]} L${dlon[0]},${dlon[1]}`;
  return (
    `<path d="${d}" fill="none" stroke="${K.granat}" stroke-width="15" stroke-linecap="round" stroke-linejoin="round"/>` +
    `<path d="${d}" fill="none" stroke="${K.zolty}" stroke-width="10.5" stroke-linecap="round" stroke-linejoin="round"/>` +
    `<circle cx="${dlon[0]}" cy="${dlon[1]}" r="6" fill="${K.skora}" ${obrys}/>`
  );
}

function oko(cx: number, typ: Oczy, prawe: boolean): string {
  const cy = 74;
  const luk = (gora: boolean) =>
    `<path d="M${cx - 6},${cy + (gora ? 2 : -1)} Q${cx},${cy + (gora ? -6 : 5)} ${cx + 6},${cy + (gora ? 2 : -1)}" fill="none" stroke="${K.granat}" stroke-width="2.6" stroke-linecap="round"/>`;
  if (typ === "zamkniete") return luk(false);
  if (typ === "radosne" || (typ === "oczko" && prawe)) return luk(true);
  const [dx, dy] = typ === "w_gore_lewo" ? [-2, -2] : typ === "w_dol" ? [0, 2] : typ === "w_bok" ? [2, 0] : [0, 0];
  const powieka =
    typ === "senne" || typ === "w_dol"
      ? `<path d="M${cx - 7.5},${cy - 1} L${cx + 7.5},${cy - 1} L${cx + 7.5},${cy - 10} L${cx - 7.5},${cy - 10} Z" fill="${K.skora}"/>` +
        `<path d="M${cx - 7},${cy - 1} L${cx + 7},${cy - 1}" stroke="${K.granat}" stroke-width="2.4" stroke-linecap="round"/>`
      : "";
  return (
    `<ellipse cx="${cx}" cy="${cy}" rx="6.5" ry="8.5" fill="${K.bialy}" ${obrys}/>` +
    `<ellipse cx="${cx + dx}" cy="${cy + dy + 1}" rx="5" ry="6.5" fill="${K.bursztyn}"/>` +
    `<ellipse cx="${cx + dx}" cy="${cy + dy + 1.5}" rx="2.6" ry="3.6" fill="${K.zrenica}"/>` +
    `<circle cx="${cx + dx + 1.8}" cy="${cy + dy - 2.5}" r="1.7" fill="${K.bialy}"/>` +
    powieka
  );
}

function usta(typ: Usta): string {
  switch (typ) {
    case "usmiech":
      return `<path d="M94,92 Q100,97 106,92" fill="none" stroke="${K.granat}" stroke-width="2.2" stroke-linecap="round"/>`;
    case "szeroki":
      return `<path d="M92,90 Q100,103 108,90 Z" fill="${K.usta}" ${obrys}/>`;
    case "otwarte":
      return `<ellipse cx="100" cy="93" rx="3.5" ry="4" fill="${K.usta}" ${obrys}/>`;
    case "jezyk":
      return (
        `<path d="M92,90 Q100,102 108,90 Z" fill="${K.usta}" ${obrys}/>` +
        `<ellipse cx="103" cy="97" rx="3.5" ry="3" fill="${K.jezyk}" ${obrys}/>`
      );
    case "spokojne":
      return `<path d="M96,93 L104,93" stroke="${K.granat}" stroke-width="2.2" stroke-linecap="round"/>`;
    case "brak":
      return "";
  }
}

function glowa(opis: Opis): string {
  const [lewe, prawe] = [oko(88, opis.oczy, false), oko(112, opis.oczy, true)];
  return (
    // kucyk z boku (lawenda z brzoskwiniową końcówką) i gumka
    `<path d="M122,36 C152,18 176,50 164,90 C160,102 150,108 146,98 C152,76 146,54 122,46 Z" fill="${K.lawenda}" ${obrys}/>` +
    `<path d="M164,88 C160,100 151,107 147,99 C150,92 152,88 152,82 Z" fill="${K.brzoskwinia}"/>` +
    `<circle cx="124" cy="40" r="4.5" fill="${K.cyjan}" ${obrys}/>` +
    // włosy z tyłu, twarz, grzywka
    `<circle cx="100" cy="64" r="41" fill="${K.lawenda}" ${obrys}/>` +
    `<path d="M62,74 C60,96 66,108 72,110 L72,80 Z" fill="${K.brzoskwinia}"/>` +
    `<circle cx="100" cy="70" r="34" fill="${K.skora}" ${obrys}/>` +
    `<path d="M65,66 Q68,30 100,29 Q132,30 135,66 Q124,50 114,58 Q107,44 96,55 Q85,45 76,62 Z" fill="${K.lawenda}" ${obrys}/>` +
    `<path d="M84,40 Q92,34 102,36" fill="none" stroke="${K.bialy}" stroke-width="2" stroke-linecap="round" opacity="0.7"/>` +
    // słuchawki z anteną zakończoną kulką
    `<path d="M57,80 C50,14 150,14 143,80" fill="none" stroke="${K.sluchawki}" stroke-width="7" stroke-linecap="round"/>` +
    `<path d="M50,72 L40,42" stroke="${K.sluchawki}" stroke-width="3" stroke-linecap="round"/>` +
    `<circle cx="39" cy="38" r="5.5" fill="${K.cyjan}" ${obrys}/>` +
    `<ellipse cx="56" cy="84" rx="10" ry="13" fill="${K.sluchawki}" stroke="${K.cyjan}" stroke-width="2.6"/>` +
    `<ellipse cx="144" cy="84" rx="10" ry="13" fill="${K.sluchawki}" stroke="${K.cyjan}" stroke-width="2.6"/>` +
    // twarz
    lewe +
    prawe +
    `<ellipse cx="81" cy="86" rx="5" ry="3" fill="${K.rumieniec}" opacity="0.75"/>` +
    `<ellipse cx="119" cy="86" rx="5" ry="3" fill="${K.rumieniec}" opacity="0.75"/>` +
    usta(opis.usta)
  );
}

function tulow(): string {
  return (
    `<path d="M68,112 Q100,100 132,112 L138,162 Q100,170 62,162 Z" fill="${K.zolty}" ${obrys}/>` +
    `<path d="M100,108 L100,164" stroke="${K.cyjan}" stroke-width="1.8"/>` +
    `<path d="M70,140 Q84,146 96,140 M104,140 Q116,146 130,140" fill="none" stroke="${K.cyjan}" stroke-width="1.6"/>` +
    `<path d="M88,106 L100,120 L112,106" fill="${K.zoltyCien}" ${obrys}/>`
  );
}

function nogiStojace(): string {
  return (
    `<rect x="77" y="158" width="46" height="18" rx="6" fill="${K.granat}"/>` +
    `<rect x="81" y="174" width="12" height="15" fill="${K.skora}" ${obrys}/>` +
    `<rect x="107" y="174" width="12" height="15" fill="${K.skora}" ${obrys}/>` +
    `<rect x="72" y="186" width="25" height="12" rx="6" fill="${K.bialy}" ${obrys}/>` +
    `<rect x="103" y="186" width="25" height="12" rx="6" fill="${K.bialy}" ${obrys}/>`
  );
}

function nogiSkrzyzowane(): string {
  return (
    `<path d="M58,182 Q100,166 142,182 Q144,196 128,197 L72,197 Q56,196 58,182 Z" fill="${K.granat}" ${obrys}/>` +
    `<path d="M70,188 Q100,178 130,188" fill="none" stroke="${K.skora}" stroke-width="7" stroke-linecap="round"/>` +
    `<ellipse cx="60" cy="190" rx="12" ry="7" fill="${K.bialy}" ${obrys}/>` +
    `<ellipse cx="140" cy="190" rx="12" ry="7" fill="${K.bialy}" ${obrys}/>`
  );
}

function rekwizyty(rece: Rece): string {
  switch (rece) {
    case "konsola":
      return (
        `<rect x="80" y="136" width="40" height="20" rx="6" fill="${K.sluchawki}" ${obrys}/>` +
        `<rect x="92" y="140" width="16" height="11" rx="2" fill="${K.cyjan}"/>`
      );
    case "kubek":
      return (
        `<path d="M101,112 L104,92" stroke="${K.granat}" stroke-width="2.6" stroke-linecap="round"/>` +
        `<path d="M89,112 L111,112 L108,136 L92,136 Z" fill="${K.brzoskwinia}" ${obrys}/>` +
        `<circle cx="96" cy="130" r="2.2" fill="${K.granat}"/><circle cx="103" cy="131" r="2.2" fill="${K.granat}"/>`
      );
    case "ekran":
      return (
        `<rect x="148" y="62" width="40" height="28" rx="4" fill="${K.cyjan}" opacity="0.35" ${obrys}/>` +
        `<path d="M154,70 L180,70 M154,77 L174,77 M154,84 L168,84" stroke="${K.granat}" stroke-width="2" stroke-linecap="round"/>`
      );
    case "wskazuje":
      return `<path d="M34,112 L24,110" stroke="${K.granat}" stroke-width="7" stroke-linecap="round"/><path d="M34,112 L24,110" stroke="${K.skora}" stroke-width="3.6" stroke-linecap="round"/>`;
    case "v":
      return (
        `<path d="M144,78 L138,62 M148,78 L152,62" stroke="${K.granat}" stroke-width="6.5" stroke-linecap="round"/>` +
        `<path d="M144,78 L138,62 M148,78 L152,62" stroke="${K.skora}" stroke-width="3.2" stroke-linecap="round"/>`
      );
    default:
      return "";
  }
}

function zzz(): string {
  const z = (x: number, y: number, s: number) =>
    `<path d="M${x},${y} l${s},0 l${-s},${s} l${s},0" fill="none" stroke="${K.lawendaCien}" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"/>`;
  return z(150, 46, 8) + z(162, 30, 10) + z(176, 12, 12);
}

/** Pełny SVG pozy 200×200; dół postaci na dolnej krawędzi. */
export function svgPozy(poza: Poza): string {
  const opis = OPISY[poza];
  const dy = opis.siedzi ? 28 : 0;
  const [lewe, prawe] = RAMIONA[opis.rece];
  const gora =
    `<g transform="rotate(${opis.przechyl ?? 0} 100 106)">${glowa(opis)}</g>` + (opis.zzz ? zzz() : "");
  const korpus = tulow() + ramie(lewe) + ramie(prawe) + rekwizyty(opis.rece);
  const ruchome = `<g transform="translate(0 ${dy}) rotate(${opis.pochylenie ?? 0} 100 160)">${korpus}${gora}</g>`;
  const nogi = opis.siedzi ? nogiSkrzyzowane() : nogiStojace();
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 200">${nogi}${ruchome}</svg>`;
}

/** Sama głowa ze słuchawkami, na ikonę. */
export function svgIkony(): string {
  const opis: Opis = { rece: "luzno", oczy: "otwarte", usta: "szeroki" };
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="22 4 160 140">${glowa(opis)}</svg>`;
}

export function adresZastepczy(poza: Poza): string {
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svgPozy(poza))}`;
}
