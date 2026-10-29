# Asystentka na pulpit

Widget na Windows 10/11: postać chibi w prawym dolnym rogu ekranu, która wita po
starcie, pokazuje plan dnia z porannego zadania, liczy czas wybranego zadania z
pauzą i zapisuje wykonaną pracę do dziennika w Obsidianie. Działa wyłącznie na
plikach na dysku, bez internetu i bez LLM.

Pełna specyfikacja: [SPEC.md](SPEC.md) (eksport dokumentu „Asystentka na pulpit —
grafiki i specyfikacja v1”). Stack: Tauri 2 (Rust) i frontend w TypeScript, HTML i
CSS bez frameworka.

## Stan budowy (kolejność z B9)

| Krok | Zakres | Stan |
| :-- | :-- | :-- |
| 1 | Okno w rogu z postacią zastępczą, autostart, zasobnik, jedna instancja | gotowe, do sprawdzenia na Windows |
| 2 | Odczyt planu i lista zadań na plikach z `przyklady/` | gotowe (`npm run piaskownica`) |
| 3 | Licznik, pauza, zapis dziennika i stanu zadań | gotowe, sprawdzone testami i na żywo |
| 4 | Zdarzenia Windows: blokada, uśpienie, bezczynność, pełny ekran, ukrycie przed nagrywaniem | kod gotowy i kompiluje się pod Windows, wymaga testu na Windows |
| 5 | Grafiki z części A, animacje i dymki | gotowe: 18 póz i ikona w `src/assets/postac/`, animacje i dymki |
| 6 | Prawdziwe foldery z B2 i zmiany w porannym zadaniu z B8 | po Twojej stronie (foldery) i w czacie (poranne zadanie) |

## Wymagania na Windows

- Windows 10 w wersji 2004 lub nowszej albo Windows 11 (WebView2 jest w systemie);
- Node.js 22 LTS;
- Rust z [rustup](https://rustup.rs) (toolchain MSVC) i Microsoft C++ Build Tools
  z zestawem „Programowanie aplikacji klasycznych w języku C++”
  ([wymagania Tauri](https://v2.tauri.app/start/prerequisites/)).

## Uruchomienie

Instalacja zależności:

```bash
npm install
```

Widget na przykładach z `przyklady/`, z dzisiejszą datą, w osobnym katalogu
`.piaskownica/` (prawdziwe ustawienia, Dysk i vault zostają nietknięte):

```bash
npm run piaskownica
```

Odmiany piaskownicy: `-- --od-nowa` czyści ją przed startem, `-- --bez-planu`
startuje bez planu (stan „Czeka na plan”), a `-- --dodaj-plan` w drugim terminalu
dopisuje dzisiejszy plan, tak jak poranne zadanie. Widget zauważy go w ciągu 30 s.

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
`powitanie`, `lista`, `czeka_na_plan`, `praca`, `pauza`, `jestes`, `gratulacje`,
`koniec_dnia`; dopisz `&strona=lewa`, żeby zobaczyć odbicie lustrzane.

## Testy

```bash
npm test
```

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

Testy w Ruście przechodzą przez cały dzień pracy z SPEC.md: powitanie, plan,
sesje z pauzą, „Jesteś?”, blokadę, awarię procesu, północ, 22:00 i niedostępny
Dysk. Sprawdzają też, że dziennik i `stan-zadan.json` mają dokładnie format z B3.
Workflow `.github/workflows/ci.yml` uruchamia testy na Linuksie i Windows oraz
buduje instalator Windows jako artefakt.

## Sprawdzenie kryteriów akceptacji na Windows

Najwygodniej w piaskownicy. W pliku `.piaskownica/AppData/Asystentka/ustawienia.json`
ustaw `"prog_bezczynnosci_min": 1` i `"przypomnienie_przerwy_min": 2`; widget
wczyta zmianę w ciągu 30 s.

1. **Autostart i powitanie:** zainstaluj wersję z `npm run tauri build`, wyloguj się
   i zaloguj. Pełne powitanie pojawia się tylko przy pierwszym starcie dnia;
   kolejny start daje „O, wróciłeś!”.
2. **Plan:** start z `--bez-planu` pokazuje „Czeka na plan”. Po
   `npm run piaskownica -- --dodaj-plan` postać sama ogłasza plan.
3. **Licznik i pauza:** kliknij zadanie, potem Pauza i Wznów.
4. **Blokada i uśpienie:** Win+L w trakcie pracy, a po odblokowaniu jest pauza i „Wracamy?”.
   Tak samo z uśpieniem komputera.
5. **Bezczynność:** zostaw mysz i klawiaturę na minutę. Pojawia się „Jesteś?”;
   „Przerwa” odejmuje ten czas.
6. **Zrobione:** gratulacje, wpis w `.piaskownica/vault/Dziennik pracy/RRRR-MM-DD.md`
   i w `.piaskownica/Dysk/Asystentka/stan-zadan.json`.
7. **Przerwij:** minuty trafiają do obu plików jako „W toku”.
8. **Awaria:** w trakcie pracy zabij proces w Menedżerze zadań i uruchom widget
   ponownie. Sesja wraca zapauzowana, z czasem do ostatniego zapisu (co 30 s).
9. **Pełny ekran:** film na pełnym ekranie albo gra chowa widget, a licznik liczy dalej.
10. **Nagrywanie:** w OBS dodaj „Przechwytywanie ekranu” (Display Capture). Przy
    zaznaczonym „Ukryj przed nagrywaniem” w menu zasobnika widgetu nie widać w
    podglądzie OBS, a na monitorze tak.
11. **Notatki w dzienniku:** dopisz coś przed blokiem i po bloku widgetu, zakończ
    kolejne zadanie. Twoje notatki zostają.
12. **Procesor w grze:** w czasie gry widget jest schowany, animacje stoją, a stan
    systemu jest sprawdzany co 2–30 s.

Log z przebiegu, także z błędami frontendu, jest w
`%LOCALAPPDATA%\pl.bozontko.asystentka\logs\asystentka.log`. Wrzuć go do czatu,
gdy coś działa inaczej niż powinno.

## Pliki widgetu

| Plik | Zawartość |
| :-- | :-- |
| `%APPDATA%\Asystentka\ustawienia.json` | ustawienia z B7; tworzone przy pierwszym starcie |
| `%APPDATA%\Asystentka\stan.json` | trwająca sesja, prawdziwy stan zadań, pozycja okna, kolejka zapisów |
| `%APPDATA%\Asystentka\dni\RRRR-MM-DD.json` | sesje danego dnia; z nich odtwarzany jest blok w dzienniku |
| plan, `stan-zadan.json`, dziennik | jak w B3; ścieżki z ustawień |

W ustawieniach ścieżki Windows wpisuj z podwójnymi ukośnikami
(`"D:\\Vault\\Dziennik pracy"`) albo z `/`. Pojedyncze ukośniki widget też naprawia,
a zmienne w stylu `%USERPROFILE%` rozwija.

## Struktura

```text
asystentka/
  SPEC.md                  specyfikacja v1 (eksport z Google Docs)
  docs/                    diagramy ze specyfikacji
  przyklady/               plan-2026-10-30.json, stan-zadan.json
  scripts/piaskownica.mjs  widget na przykładach w .piaskownica/
  src/                     frontend: TypeScript, HTML, CSS
    assets/postac/         grafiki z części A i opis postac.json (zasady w README.md tam)
    kwestie.json           dymki
    main.ts                rysowanie migawek, dymki, efekty, rozmiar okna
    widok.ts               panele stanów, licznik
    postac.ts              pozy, mrugnięcia, klatki, zmiany pozy
    grafiki.ts             pliki póz i rozmiar postaci z postac.json
    zastepcza.ts           postać zastępcza w SVG (gdy brak PNG)
  src-tauri/               backend: Rust
    src/rdzen.rs           stany postaci i przejścia (B4, B5), bez zależności od Tauri
    src/sesja.rs           sesja: odcinki pracy, pauzy, bezczynność, odtworzenie po awarii
    src/plan.rs            wyszukiwanie i odczyt planu dnia
    src/zadania.rs         stan-zadan.json
    src/dziennik.rs        dziennik w Obsidianie: frontmatter i blok widgetu
    src/aplikacja.rs       okno, zasobnik, skrót, zdarzenia, wątek terminów
    src/system/win.rs      Windows: WTS, zasilanie, GetLastInputInfo, pełny ekran, WDA_EXCLUDEFROMCAPTURE
```

## Decyzje i doprecyzowania względem SPEC.md

- **Pliki obsługuje Rust**, a nie wtyczka file-system. Zapis atomowy, kolejka
  ponowień co 30 s i praca przy schowanym oknie są prostsze w backendzie, a
  frontend nie dostaje dostępu do dysku.
- **Pozycję okna zapamiętuje widget**, a nie wtyczka window-state. Okno zmienia
  rozmiar ze stanem i ma trzymać dolną krawędź, więc zapisywana jest odległość od
  krawędzi obszaru roboczego, a nie lewy górny róg. Po przeciągnięciu na lewą
  połowę ekranu postać odbija się lustrzanie, a panel przechodzi na drugą stronę.
- **Dodatkowe oficjalne wtyczki:** opener (otwiera plik ustawień z zasobnika) i log
  (plik logu do diagnozy).
- **Okno nie kradnie fokusu:** jest niefokusowalne (`focusable: false`) i pokazuje
  się bez aktywacji. Jeśli na jakimś Windows kliknięcia w panel nie działają,
  pierwsza rzecz do sprawdzenia to ta flaga w `src-tauri/tauri.conf.json`.
- **Stan zadań:** prawdę widget trzyma lokalnie w `stan.json` i po każdej zmianie
  zapisuje kopię na Dysk. Format pliku na Dysku jest dokładnie jak w B3. Przy
  starcie i przed zapisem przejmuje z Dysku zadania, których nie zna lokalnie,
  np. po reinstalacji.
- **„Czeka na plan”** pokazuje zadania „W toku” z nazwami z wcześniejszych planów.
  Można je kliknąć i pracować, zanim przyjdzie nowy plan.
- **Krótkie sesje:** „Przerwij” przed upływem minuty niczego nie zapisuje (ochrona
  przed przypadkowym kliknięciem). „Zrobione” zapisuje zawsze.
- **Północ:** trwająca sesja liczy się dalej i należy do dnia startu, jak w B5.
  Sesja zapauzowana z poprzedniego dnia trafia do dziennika tamtego dnia jako
  „W toku”, a nowy dzień zaczyna się czysto.
- **Blokada przy wiszącym „Jesteś?”:** nikogo nie było, więc pauza liczy się od
  ostatniej aktywności. Jeśli Windows nie zgłosi uśpienia, przerwa w działaniu
  procesu dłuższa niż 90 s też pauzuje sesję od ostatniego sprawdzenia.
- **Szacunek obok licznika** to minuty z dzisiejszych sesji tego zadania razem z
  bieżącą, np. „47 / 60 min”. Duży licznik pokazuje bieżącą sesję.
- **Koniec dnia:** o 22:00 tylko raz dziennie. Panel ma link „Wróć do listy”, np. do
  zadań zapasowych po skończeniu planu.
- **Dodatkowe dymki** w `src/kwestie.json`: `powitanie_bez_planu`, `lista`,
  `plan_dotarl`, `przerwij`, `blad_ustawien`. Test pilnuje, żeby każdy dymek z
  backendu miał tam wariant.
- **Ustawienia:** zmiany w pliku są wczytywane same co 30 s (także skrót).
  „Ukryj przed nagrywaniem” w zasobniku zapisuje się do pliku. Uszkodzony plik nie
  jest nadpisywany; zostają poprzednie ustawienia, a postać o tym mówi.
- **Postać:** rozmiar (228 × 280 px), linie stóp i siedziska widget bierze z
  `src/assets/postac/postac.json`. W Pracy i Pauzie siedzi na górnej krawędzi
  panelu, a reszta postaci chowa się za nim. Jest wysoka jak wariant B z A2b, więc
  oddycha delikatniej (skala do 1,008). Inny zestaw grafik wystarczy podmienić
  razem z jego `postac.json`.
