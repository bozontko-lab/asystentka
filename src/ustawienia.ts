// Okno ustawień (SPEC.md, B6): pola z kreatora i pozostałe ustawienia z B7.
// Zapisz stosuje zmiany od razu, bez restartu.

import "./kolory.css";
import "./formularz.css";
import "./ustawienia.css";

import { el } from "./dom";
import { godzinyNaMinuty, minutyNaGodziny } from "./kreator";
import { type Most, mostTauri, wTauri } from "./most";
import { mostPodglad } from "./podglad";
import { PoleSkrotu } from "./skrot";
import type { Ustawienia } from "./typy";

const most: Most = wTauri() ? mostTauri() : mostPodglad();
const opisBledu = (e: unknown) => (typeof e === "string" ? e : e instanceof Error ? e.message : "Nieznany błąd.");

function pole(id: string, etykieta: string, wejscie: HTMLElement, podpowiedz?: string): HTMLElement {
  return el(
    "div",
    { class: "pole" },
    el("label", { for: id }, etykieta),
    wejscie,
    podpowiedz && el("span", { class: "podpowiedz" }, podpowiedz),
  );
}

function tekst(id: string, wartosc: string): HTMLInputElement {
  const w = el("input", { id, type: "text", spellcheck: "false" });
  w.value = wartosc;
  return w;
}

function liczba(id: string, wartosc: number, min: number, max: number): HTMLInputElement {
  const w = el("input", { id, type: "number", min: String(min), max: String(max), step: "1" });
  w.value = String(wartosc);
  return w;
}

function przelacznik(id: string, etykieta: string, opis: string, wlaczony: boolean): [HTMLElement, HTMLInputElement] {
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

function sciezka(
  id: string,
  etykieta: string,
  wartosc: string,
  przycisk: string,
  wybor: (obecny: string) => Promise<string | null>,
  podpowiedz?: string,
): [HTMLElement, HTMLInputElement] {
  const w = tekst(id, wartosc);
  const b = el("button", { class: "przycisk", type: "button" }, przycisk);
  b.addEventListener("click", () => {
    void wybor(w.value).then((wybrane) => {
      if (wybrane) w.value = wybrane;
    });
  });
  return [pole(id, etykieta, el("div", { class: "wiersz-sciezki" }, w, b), podpowiedz), w];
}

async function start(): Promise<void> {
  const u = await most.ustawieniaPobierz();
  const korzen = document.getElementById("ustawienia");
  if (!korzen) return;

  const imie = tekst("imie", u.imie);
  const postac = tekst("postac", u.nazwa_postaci);
  postac.maxLength = 30;
  const nazwaPostaci = u.nazwa_postaci.trim() || "Sai";
  document.title = `${nazwaPostaci} · ustawienia`;
  const [poleZadan, plikZadan] = sciezka(
    "plik-zadan",
    "Plik zadań",
    u.plik_zadan,
    "Wskaż plik…",
    (obecny) => most.wybierzPlik(obecny),
    "Zwykły plik Markdown, np. Zadania.md w vaulcie Obsidiana.",
  );
  const [poleDziennika, folderDziennika] = sciezka(
    "folder-dziennika",
    "Folder dziennika",
    u.folder_dziennika,
    "Wybierz folder…",
    (obecny) => most.wybierzFolder(obecny, "Folder dziennika pracy"),
    "Każdy dzień to osobna notatka RRRR-MM-DD.md.",
  );
  const cel = tekst("cel", minutyNaGodziny(u.cel_dzienny_min));
  cel.inputMode = "decimal";
  const koniec = el("input", { id: "koniec", type: "time" });
  koniec.value = u.godzina_konca_dnia;
  const [poleAutostartu, autostart] = przelacznik(
    "autostart",
    "Uruchamiaj razem z Windows",
    "Start po zalogowaniu, z powitaniem raz dziennie.",
    u.autostart,
  );
  const bezczynnosc = liczba("bezczynnosc", u.prog_bezczynnosci_min, 0, 240);
  const przerwa = liczba("przerwa", u.przypomnienie_przerwy_min, 0, 480);
  const [poleNagrywania, nagrywanie] = przelacznik(
    "nagrywanie",
    "Ukryj przed nagrywaniem",
    "Widget jest widoczny na monitorze, ale nie w OBS ani na udostępnianym ekranie.",
    u.ukryj_przed_nagrywaniem,
  );
  const [polePelnegoEkranu, pelnyEkran] = przelacznik(
    "pelny-ekran",
    "Chowaj przy pełnym ekranie",
    "Gry i filmy na pełnym ekranie chowają widget; licznik liczy dalej.",
    u.chowaj_przy_pelnym_ekranie,
  );
  const poleSkrotu = new PoleSkrotu(most, u.skrot_pokaz_ukryj);

  const blad = el("div", { class: "blad-formularza", role: "alert", hidden: true });
  const zapisz = el("button", { class: "przycisk glowny", type: "button" }, "Zapisz");
  const anuluj = el("button", { class: "przycisk", type: "button" }, "Anuluj");

  const zbierz = (): Ustawienia | string => {
    const celMin = godzinyNaMinuty(cel.value);
    if (celMin === null) return "Cel dzienny to liczba godzin od 0,25 do 24.";
    return {
      ...u,
      imie: imie.value.trim(),
      nazwa_postaci: postac.value.trim(),
      plik_zadan: plikZadan.value.trim(),
      folder_dziennika: folderDziennika.value.trim(),
      cel_dzienny_min: celMin,
      godzina_konca_dnia: koniec.value,
      autostart: autostart.checked,
      prog_bezczynnosci_min: Number.parseInt(bezczynnosc.value || "0", 10),
      przypomnienie_przerwy_min: Number.parseInt(przerwa.value || "0", 10),
      ukryj_przed_nagrywaniem: nagrywanie.checked,
      chowaj_przy_pelnym_ekranie: pelnyEkran.checked,
      skrot_pokaz_ukryj: poleSkrotu.skrot,
    };
  };

  const pokazBlad = (tresc: string) => {
    blad.textContent = tresc;
    blad.hidden = tresc === "";
  };

  zapisz.addEventListener("click", () => {
    const nowe = zbierz();
    if (typeof nowe === "string") return pokazBlad(nowe);
    pokazBlad("");
    zapisz.disabled = true;
    most
      .ustawieniaZapisz(nowe)
      .then(() => most.zamknijOkno())
      .catch((e) => {
        pokazBlad(opisBledu(e));
        zapisz.disabled = false;
      });
  });
  anuluj.addEventListener("click", () => void most.zamknijOkno());
  korzen.addEventListener("keydown", (e) => {
    if (e.key === "Escape") void most.zamknijOkno();
  });

  korzen.replaceChildren(
    el("h1", {}, `${nazwaPostaci} · ustawienia`),
    el("h2", {}, "Ty"),
    pole("imie", "Jak mam się do Ciebie zwracać", imie, "Forma wstawiana do dymków bez zmian, np. w wołaczu."),
    pole(
      "postac",
      "Imię postaci",
      postac,
      "Tak nazywa się w dymkach, menu i oknach. Może mieć też znaki japońskie, np. 才ちゃん.",
    ),
    el("h2", {}, "Pliki"),
    poleZadan,
    poleDziennika,
    el("h2", {}, "Dzień pracy"),
    pole("cel", "Dzienny cel (godziny)", cel, "Do niego porównuję sumę zadań i przepracowany czas."),
    pole("koniec", "Zamknięcie dnia o godzinie", koniec, "Wtedy sam podsumowuję dzień, jeśli nie zrobiłeś tego wcześniej."),
    pole("bezczynnosc", "Pytaj „Przerwa czy praca?” po (min bez ruchu)", bezczynnosc, "0 wyłącza pytanie."),
    pole("przerwa", "Przypomnij o przerwie po (min pracy)", przerwa, "0 wyłącza przypomnienie."),
    el("h2", {}, "Okno i system"),
    poleAutostartu,
    poleNagrywania,
    polePelnegoEkranu,
    poleSkrotu.korzen,
    blad,
    el("div", { class: "przyciski" }, anuluj, zapisz),
  );
  imie.focus();
}

start().catch((e) => most.zglosBlad(opisBledu(e)));
