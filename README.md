# Asystentka na pulpit

Widget na Windows 10/11: postać chibi w prawym dolnym rogu ekranu, która wita po
starcie, pozwala raz dziennie wybrać zadania z `Zadania.md`, liczy czas wybranego
zadania z pauzą, dopisuje czas i ptaszka do pliku zadań i zapisuje dzień do
dziennika w Obsidianie. Na koniec dnia pokazuje podsumowanie i reaguje na to, ile
udało się zrobić. Działa wyłącznie na plikach na dysku, bez internetu i bez LLM.

Pełna specyfikacja: [SPEC.md](SPEC.md) (wersja 2). Stack: Tauri 2 (Rust) i frontend
w TypeScript, HTML i CSS bez frameworka.

## Stan wdrożenia wersji 2 (kolejność z B9)

| Krok | Zakres | Stan |
| :-- | :-- | :-- |
| 1 | Ustawienia v2, okienko ustawień i kreator (z przejściem ustawień z v1) | gotowe, sprawdzone na żywo |
| 2 | Odczyt i zapis `Zadania.md`, obserwacja zmian pliku | gotowe, testy na `przyklady/` i na żywo |
| 3 | Wybór zadań, `dzien.json`, Zmień plan, Dodaj zadanie; bez planu z Dysku i „Czeka na plan” | gotowe, sprawdzone na żywo |
| 4 | Podsumowanie po zadaniu, zapis sesji do pliku zadań i dziennika | gotowe, sprawdzone na żywo |
| 5 | Koniec dnia: przycisk, zamknięcie o 22:00, sześć reakcji, podsumowanie w dzienniku, „Wróć do pracy” | gotowe, sprawdzone na żywo |
| 6 | Menu pod prawym przyciskiem, menu zasobnika, „Zamknij asystentkę” | gotowe, sprawdzone na żywo |

Zdarzenia Windows (blokada, uśpienie, bezczynność, pełny ekran, ukrycie przed
nagrywaniem) działają w kodzie dla Windows i kompilują się w CI, ale tylko Ty
możesz je sprawdzić na prawdziwym systemie; patrz „Sprawdzenie na Windows”.

## Wymagania na Windows

- Windows 10 w wersji 2004 lub nowszej albo Windows 11 (WebView2 jest w systemie);
- Node.js 26 LTS (wystarczy też 22.12 lub nowszy);
- Rust z [rustup](https://rustup.rs) (toolchain MSVC) i Microsoft C++ Build Tools
  z zestawem „Programowanie aplikacji klasycznych w języku C++”
  ([wymagania Tauri](https://v2.tauri.app/start/prerequisites/)).

## Uruchomienie

```bash
npm install
```

Widget na przykładach z `przyklady/`, z dzisiejszą datą, w osobnym katalogu
`.piaskownica/` (prawdziwe ustawienia i vault zostają nietknięte):

```bash
npm run piaskownica
```

Odmiany piaskownicy:

| Polecenie | Efekt |
| :-- | :-- |
| `npm run piaskownica -- --od-nowa` | czyści piaskownicę przed startem |
| `npm run piaskownica -- --v1` | zostawia ustawienia z wersji 1; kreator wypełnia się nimi |
| `npm run piaskownica -- --kreator` | bez pliku ustawień, kreator od zera |
| `npm run piaskownica -- --tylko-pliki` | przygotowuje pliki bez uruchamiania |

Widget na prawdziwych ustawieniach z `%APPDATA%\Asystentka\ustawienia.json`:

```bash
npm run tauri dev
```

Instalator (NSIS, instalacja dla bieżącego użytkownika) trafia do
`src-tauri/target/release/bundle/nsis/`:

```bash
npm run tauri build
```

Autostart włącza się tylko w wersji zainstalowanej, nie w `tauri dev`.

Wygląd wszystkich stanów można obejrzeć w zwykłej przeglądarce, bez Tauri:
`npm run dev`, a potem `http://localhost:1420/?stan=praca`. Dostępne stany:
`konfiguracja`, `powitanie`, `wybor_zadan`, `lista`, `praca`, `pauza`, `jestes`,
`gratulacje`, `koniec_dnia`. Parametry: `&strona=lewa` (odbicie lustrzane),
`&suma=duza` (przekroczony cel w Wyborze), `&plan=wykonany`, `&etap=`/`&reakcja=`
w końcu dnia.

## Pierwsze uruchomienie po aktualizacji z wersji 1

Brak pliku ustawień albo pusta ścieżka do pliku zadań otwiera **kreator**. Kreator
jest od razu wypełniony Twoimi ustawieniami z wersji 1: folder dziennika, cel dnia,
godzina końca dnia, autostart i skrót zostają, a plik zadań proponowany jest w
Twoim vaulcie (vaulty czyta z `obsidian.json`). Nowy plik powstaje z szablonu
i otwiera się w Obsidianie; istniejącego pliku widget nigdy nie nadpisuje. Stare
klucze (`plik_planu`, `plik_stanu`) zostają w pliku ustawień nietknięte, ale nie są
już używane. Sesje z `dni/*.json` i pozycja okna ze `stan.json` są przenoszone do
`dzien.json` i `okno.json`.

## Testy

```bash
npm test
```

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

Testy w Ruście (117) przechodzą przez dzień pracy: powitanie, Wybór zadań, plan,
sesje z pauzą, „Jesteś?”, blokadę, awarię procesu, północ, koniec dnia z sześcioma
reakcjami, zamknięcie o 22:00 i zaległe podsumowanie wczorajszego dnia. Parser
`Zadania.md` ma osobne testy: kolejność pól, CRLF, BOM, niezmieniona reszta
pliku. Workflow `.github/workflows/ci.yml`
uruchamia testy na Linuksie i Windows oraz buduje instalator Windows jako artefakt.

## Sprawdzenie na Windows

Najwygodniej w piaskownicy. W `.piaskownica/AppData/Asystentka/ustawienia.json`
ustaw `"prog_bezczynnosci_min": 1` i `"przypomnienie_przerwy_min": 2`, albo zmień je
w okienku Ustawienia (działa od razu).

1. **Kreator:** `npm run piaskownica -- --od-nowa --v1` pokazuje kreator
   wypełniony ustawieniami z v1; „Utwórz plik” zakłada `Zadania.md` i otwiera go
   w Obsidianie.
2. **Ustawienia:** prawy przycisk, Ustawienia. Zmiana imienia czy celu działa bez restartu.
3. **Wybór zadań:** pierwszy start dnia pokazuje zadania z zaznaczonymi „w toku”;
   po przekroczeniu celu o 20% suma robi się koralowa. Restart wraca do Listy.
4. **Obserwacja pliku:** zapisz `Zadania.md` w Obsidianie, lista odświeża się w sekundę.
5. **Zrobione i Przerwij:** zmienia dokładnie jedną linię w pliku; Przerwij tylko `[czas:: …]`.
6. **Dodaj zadanie:** dopisuje linię pod wybranym nagłówkiem i do planu.
7. **Koniec pracy na dziś:** podsumowanie, reakcja z tabeli, sekcja w dzienniku;
   „Wróć do pracy” usuwa ją z dziennika.
8. **22:00:** ustaw `godzina_konca_dnia` kilka minut do przodu; dzień zamyka się
   sam, także z trwającą sesją. Po wyłączeniu komputera podsumowanie wczoraj
   pojawia się przy następnym starcie.
9. **Zamknij asystentkę:** przy działającym liczniku zapisuje sesję i kończy proces.
10. **Blokada i uśpienie:** Win+L w trakcie pracy, po odblokowaniu pauza i „Wracamy?”.
11. **Bezczynność:** minuta bez ruchu, „Jesteś?”; „Przerwa” odejmuje ten czas.
12. **Awaria:** zabij proces w trakcie pracy; sesja wraca zapauzowana, bez przestoju.
13. **Pełny ekran:** film lub gra chowa widget, licznik liczy dalej.
14. **Nagrywanie:** w OBS „Przechwytywanie ekranu”; przy „Ukryj przed nagrywaniem”
    widgetu nie widać w OBS, a na monitorze tak.
15. **Notatki w dzienniku:** wszystko poza blokiem widgetu zostaje nietknięte.

Log z przebiegu, także z błędami frontendu, jest w
`%LOCALAPPDATA%\pl.bozontko.asystentka\logs\asystentka.log`. Wrzuć go do czatu,
gdy coś działa inaczej niż powinno.

## Pliki widgetu

| Plik | Zawartość |
| :-- | :-- |
| `%APPDATA%\Asystentka\ustawienia.json` | ustawienia z B7; zmieniane w okienku Ustawienia |
| `%APPDATA%\Asystentka\dzien.json` | plan dnia, trwająca sesja, sesje dnia, kolejka zapisów |
| `%APPDATA%\Asystentka\okno.json` | pozycja okna |
| `Zadania.md` | zadania; widget dopisuje `[czas:: N]`, `[x]` i `✅ data` |
| `<folder dziennika>\RRRR-MM-DD.md` | dziennik z blokiem widgetu i podsumowaniem dnia |

Ustawienia edytuje się w okienku, ale plik JSON nadal można zmienić ręcznie.
Ścieżki Windows wpisuj z podwójnymi ukośnikami albo z `/`.

## Struktura

```text
asystentka/
  SPEC.md                  specyfikacja v2
  przyklady/               Zadania.md i pusty Dziennik pracy/ do testów
  scripts/piaskownica.mjs  widget na przykładach w .piaskownica/
  src/                     frontend: TypeScript, HTML, CSS
    assets/postac/         grafiki z części A i opis postac.json
    kwestie.json           dymki
    main.ts                rysowanie migawek, dymki, efekty, rozmiar okna
    widok.ts               panele stanów, licznik
    kreator.ts             kreator pierwszego uruchomienia
    dodawanie.ts           formularz „Dodaj zadanie”
    ustawienia.ts/.html    okienko ustawień
    postac.ts, grafiki.ts  pozy, reakcje, mrugnięcia
  src-tauri/               backend: Rust
    src/rdzen.rs           stany i przejścia (B4, B5), bez zależności od Tauri
    src/plik_zadan.rs      parser i zapis Zadania.md
    src/sesja.rs           sesja: odcinki pracy, pauzy, bezczynność, odtworzenie po awarii
    src/dzien.rs           dzien.json, reakcje końca dnia
    src/dziennik.rs        dziennik w Obsidianie: frontmatter, blok widgetu, podsumowanie
    src/ustawienia.rs      ustawienia v2, przejście z v1, walidacja
    src/obsidian.rs        vaulty, adresy obsidian://, szablon pliku zadań
    src/aplikacja.rs       okna, obserwator pliku, skrót, wątek terminów
    src/zasobnik.rs        menu zasobnika i menu pod prawym przyciskiem
    src/system/win.rs      Windows: WTS, zasilanie, GetLastInputInfo, pełny ekran, WDA_EXCLUDEFROMCAPTURE
```

## Decyzje i doprecyzowania względem SPEC.md

- **Pliki obsługuje Rust**, a nie wtyczka file-system. Zapis atomowy (plik tymczasowy
  i zmiana nazwy), świeży odczyt tuż przed zapisem, zachowanie BOM i CRLF, zmiana
  jednej linii, kolejka ponowień co 30 s, gdy plik jest zajęty. Obserwację zmian robi
  crate `notify` (opóźnienie 300 ms).
- **Klucz zadania** to projekt (nagłówek `##`) i tekst bez pól `[…:: …]`.
- **Pozycję okna zapamiętuje widget**, a nie wtyczka window-state: okno zmienia
  rozmiar ze stanem i trzyma dolną krawędź. Po przeciągnięciu na lewą połowę ekranu
  postać odbija się lustrzanie.
- **Dodatkowe oficjalne wtyczki:** dialog (wybór pliku i folderu), opener, log.
- **Okno nie kradnie fokusu** (`focusable: false`). Pola tekstowe (kreator, Dodaj
  zadanie) włączają fokus tylko na czas wpisywania.
- **Menu pod prawym przyciskiem** to natywne menu systemowe, pozycje są wyszarzone,
  gdy polecenie nie ma sensu (np. Zmień plan w czasie pracy).
- **Krótkie sesje:** „Przerwij” przed upływem minuty niczego nie zapisuje. „Zrobione”
  zapisuje zawsze.
- **Północ:** trwająca sesja liczy się dalej i należy do dnia startu. Poprzedni dzień
  zamyka się automatycznie z podsumowaniem (reakcja z tabeli B4).
- **Dodatkowe dymki** w `src/kwestie.json`: `lista`, `blad_ustawien`. Test pilnuje,
  żeby każdy dymek z backendu miał tam wariant.
- **Postać:** rozmiar, linie stóp i siedziska z `src/assets/postac/postac.json`;
  `concerned.png` służy reakcji „zmartwiona”.
