# Asystentka na pulpit

Widget na Windows 10/11: postać chibi w prawym dolnym rogu ekranu, która wita po
starcie, pozwala raz dziennie wybrać zadania z `Zadania.md`, liczy czas wybranego
zadania z pauzą, dopisuje czas i ptaszka do pliku zadań i zapisuje dzień do
dziennika w Obsidianie. Na koniec dnia pokazuje podsumowanie i reaguje na to, ile
udało się zrobić. Działa wyłącznie na plikach na dysku, bez internetu i bez LLM.

Pełna specyfikacja: [SPEC.md](SPEC.md) (wersja 2.1). Stack: Tauri 2 (Rust) i frontend
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

## Stan wdrożenia wersji 2.1 (kolejność z B9)

| Krok | Zakres | Stan |
| :-- | :-- | :-- |
| 1 | Wiele monitorów: kotwica w `okno.json`, rozwijanie od górnej krawędzi, skalowanie, „Przywróć pozycję”, pełny ekran tylko na monitorze widgetu | gotowe; sprawdzone na żywo na symulowanych monitorach, drugi prawdziwy monitor i 150% do sprawdzenia na Windows |
| 2 | Zrobione od razu w pliku, Cofnij, kwestia `cofniete` | gotowe, sprawdzone na żywo; odświeżenie w Obsidianie do sprawdzenia u Ciebie |
| 3 | Skrót Ctrl+Alt+Shift+Spacja, zamiana starego, `skrot_zajety`, nagrywanie skrótu w ustawieniach | gotowe; konflikt sprawdzony na żywo, rejestracja w Windows do sprawdzenia u Ciebie |
| 4 | Sekwencja przerwy, konsolka, kwestia `pauza_gra` | gotowe, sprawdzone na żywo z zegarem sterowanym przez Playwright |
| 5 | Imię postaci: `nazwa_postaci`, `{postac}` | gotowe, sprawdzone na żywo |
| 6 | Kolory ze zmiennych CSS z B7 w jednym pliku `kolory.css` | gotowe; test pilnuje palety, kontrastów i braku kolorów wpisanych na sztywno |
| 7 | Dźwięk skupienia 40 Hz (`rodio`), przełączniki, cisza przy OBS, znacznik w dzienniku | gotowe; nagranie z wirtualnego wyjścia, słuchawki i prawdziwy OBS do sprawdzenia u Ciebie |
| 8 | Głos (B10) | nie wdrożony: czeka na pliki w `src/assets/glos/` |

Dwa błędy z wersji 2 miały przyczyny, które poprawka usuwa:

- **Widget wracał na monitor główny**, bo położenie okna liczono zawsze względem
  monitora głównego, a `okno.json` pamiętał tylko bok i odstęp od dołu. Każda zmiana
  rozmiaru okna albo puszczenie widgetu po przeciągnięciu odsyłały go z powrotem.
  Teraz `okno.json` pamięta monitor, róg i odstępy w pikselach logicznych.
- **Ptaszek nie pojawiał się od razu w `Zadania.md`**, bo gdy plik był przez moment
  zajęty (Obsidian, antywirus, synchronizacja), zamiana nazwy kończyła się błędem po
  trzech próbach w 0,3 s, a zapis czekał w kolejce ponawianej dopiero co 30 s. W
  piaskownicy z zablokowanym plikiem ptaszek pojawiał się po 28 s. Teraz ponowienia
  przychodzą po 1, 1, 2, 3, 5 i 10 s, a log zapisuje czas każdego zapisu.

Zdarzenia Windows (blokada, uśpienie, bezczynność, pełny ekran, ukrycie przed
nagrywaniem, wykrywanie OBS) działają w kodzie dla Windows i kompilują się w CI, ale
tylko Ty możesz je sprawdzić na prawdziwym systemie; patrz „Sprawdzenie na Windows”.

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
`&pion=gora` (widget przy górnej krawędzi, panel w dół), `&suma=duza` (przekroczony
cel w Wyborze), `&plan=wykonany`, `&etap=`/`&reakcja=` w końcu dnia, `&pauza_s=`
(sekundy przerwy: 2 przeciąganie, 60 konsolka, 1000 drzemka), `&cofnij=nie` (dymek
gratulacji bez Cofnij), `&dzwiek=wylaczony` albo `&dzwiek=obs` (przygaszona etykieta
„♪ 40 Hz”). Okienko ustawień: `http://localhost:1420/ustawienia.html`.

### Drugi monitor bez drugiego monitora

Wersja deweloperska (`tauri dev`, `cargo build`, nie instalator) czyta układ monitorów
z pliku, jeśli ustawisz zmienną `ASYSTENTKA_MONITORY`. Jedna linia na monitor:
`nazwa x y szerokość wysokość skalowanie [wysokość_paska_zadań] [glowny]`, np.

```text
lewy   0    0 1600 1000 1.0
prawy  1600 0 1920 1080 1.5 40 glowny
```

Zmiana pliku w trakcie działania udaje podłączenie i odłączenie monitora. Służy do
testów na Linuksie, gdzie wirtualny ekran ma jeden monitor; w instalatorze tej
zmiennej nie ma.

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

Testy w Ruście (232 plus jeden ręczny) przechodzą przez dzień pracy: powitanie, Wybór zadań, plan,
sesje z pauzą, „Jesteś?”, blokadę, awarię procesu, północ, koniec dnia z sześcioma
reakcjami, zamknięcie o 22:00 i zaległe podsumowanie wczorajszego dnia. Parser
`Zadania.md` ma osobne testy: kolejność pól, CRLF, BOM, niezmieniona reszta
pliku. Workflow `.github/workflows/ci.yml`
uruchamia testy na Linuksie i Windows oraz buduje instalator Windows jako artefakt.
Na Linuksie `rodio` potrzebuje nagłówków ALSA (`libasound2-dev`).

Dźwięk ma osobne testy bez urządzenia audio: głośność w czasie, otwieranie i zamykanie
urządzenia na atrapie, rampa przy każdej próbce, 200 i 240 Hz w pliku oraz łączenie
pętli. Jeden test jest ręczny i gra scenariusz na prawdziwym wyjściu systemu (20 s):

```bash
cargo test --manifest-path src-tauri/Cargo.toml urzadzenie_gra_scenariusz -- --ignored --nocapture
```

Przy pustym wyjściu (np. wirtualne wyjście PulseAudio) można nagrać wynik i obejrzeć
obwiednię głośności, jak w sprawdzeniu wersji 2.1.

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

### Wersja 2.1: to, czego nie da się sprawdzić bez Windows

1. **Drugi monitor:** przeciągnij widget na drugi monitor; zostaje tam po rozwinięciu
   listy, starcie pracy, pauzie, końcu dnia i po restarcie aplikacji.
2. **Skalowanie 150%:** postać ma tę samą wielkość na monitorze 100% i 150%; panel
   przy górnej krawędzi rozwija się w dół i nie wychodzi poza ekran.
3. **Odłączenie monitora:** widget staje w domyślnym miejscu na głównym; „Przywróć
   pozycję” (menu pod prawym przyciskiem i zasobnik) robi to na żądanie.
4. **Pełny ekran na jednym monitorze:** gra na drugim monitorze nie chowa widgetu,
   gra na monitorze widgetu go chowa.
5. **Obsidian:** przy otwartym `Zadania.md` ptaszek i data pojawiają się w ciągu 2 s od
   kliknięcia Zrobione. Gdy nie, sprawdź w logu czas zapisu („plik zadań: … (N ms)”).
6. **Skrót:** Ctrl+Alt+Spacja dalej otwiera okno Claude, Ctrl+Alt+Shift+Spacja
   pokazuje i chowa widget. Przy zajętym skrócie widget mówi o tym w dymku, a w
   ustawieniach da się nagrać inny (kliknij pole i naciśnij kombinację).
7. **Zasobnik:** podpowiedź ikony i „Zamknij {imię}” zmieniają się po zmianie imienia
   postaci w ustawieniach, bez restartu.
8. **Dźwięk w słuchawkach:** godzina pętli bez trzasków na łączeniu; narasta przy
   starcie i wznowieniu, cichnie przy Pauzie, Przerwij, Zrobione, końcu dnia,
   blokadzie ekranu, uśpieniu i w „Jesteś?”. „Odsłuchaj” w ustawieniach gra 5 s;
   suwakiem można zmieniać głośność w trakcie.
9. **OBS:** przy działającym `obs64.exe` i włączonej ciszy przy nagrywaniu dźwięk milknie
   w ciągu kilku sekund, a etykieta „♪ 40 Hz” w panelu Pracy jest przygaszona; po
   zamknięciu OBS dźwięk wraca z narastaniem.
10. **Własny dźwięk:** plik `skupienie.wav` albo `skupienie.mp3` w
    `%APPDATA%\Asystentka\` gra zamiast wbudowanego (po następnym starcie dźwięku).
11. **Słuchawki wypięte w trakcie:** dźwięk otwiera się od nowa na bieżącym
    urządzeniu przy następnej okazji; log zapisuje „urządzenie audio otwarte w N ms”.

Log z przebiegu, także z błędami frontendu, jest w
`%LOCALAPPDATA%\pl.bozontko.asystentka\logs\asystentka.log`. Wrzuć go do czatu,
gdy coś działa inaczej niż powinno.

## Pliki widgetu

| Plik | Zawartość |
| :-- | :-- |
| `%APPDATA%\Asystentka\ustawienia.json` | ustawienia z B7; zmieniane w okienku Ustawienia |
| `%APPDATA%\Asystentka\dzien.json` | plan dnia, trwająca sesja, sesje dnia, kolejka zapisów |
| `%APPDATA%\Asystentka\okno.json` | monitor, róg i odstępy okna (stare `strona` i `odstep_dol` są przejmowane) |
| `%APPDATA%\Asystentka\skupienie.wav` lub `.mp3` | opcjonalnie: własny dźwięk skupienia zamiast wbudowanego |
| `Zadania.md` | zadania; widget dopisuje `[czas:: N]`, `[x]` i `✅ data` |
| `<folder dziennika>\RRRR-MM-DD.md` | dziennik z blokiem widgetu i podsumowaniem dnia |

Ustawienia edytuje się w okienku, ale plik JSON nadal można zmienić ręcznie.
Ścieżki Windows wpisuj z podwójnymi ukośnikami albo z `/`.

## Struktura

```text
asystentka/
  SPEC.md                  specyfikacja v2.1
  docs/                    makieta kolorów z B7
  przyklady/               Zadania.md i pusty Dziennik pracy/ do testów
  scripts/piaskownica.mjs  widget na przykładach w .piaskownica/
  src/                     frontend: TypeScript, HTML, CSS
    assets/postac/         grafiki z części A i opis postac.json
    assets/dzwiek/         skupienie_40hz.wav (wbudowany w program przez include_bytes!)
    kolory.css             zmienne z paletą z B7: jedyne miejsce z kolorami
    kwestie.json           dymki
    main.ts                rysowanie migawek, dymki, efekty, rozmiar okna
    widok.ts               panele stanów, licznik
    kreator.ts             kreator pierwszego uruchomienia
    dodawanie.ts           formularz „Dodaj zadanie”
    ustawienia.ts/.html    okienko ustawień
    postac.ts, grafiki.ts  pozy, reakcje, mrugnięcia
    przerwa.ts             czasy sekwencji przerwy i konsolki
    skrot.ts               pole nagrywania skrótu w ustawieniach
    dzwiek.ts              suwak głośności i „Odsłuchaj” w ustawieniach
  src-tauri/               backend: Rust
    src/rdzen.rs           stany i przejścia (B4, B5), bez zależności od Tauri
    src/plik_zadan.rs      parser i zapis Zadania.md
    src/sesja.rs           sesja: odcinki pracy, pauzy, bezczynność, odtworzenie po awarii
    src/dzien.rs           dzien.json, reakcje końca dnia
    src/dziennik.rs        dziennik w Obsidianie: frontmatter, blok widgetu, podsumowanie
    src/ustawienia.rs      ustawienia v2, przejście z v1, walidacja
    src/obsidian.rs        vaulty, adresy obsidian://, szablon pliku zadań
    src/aplikacja.rs       okna, obserwator pliku, skrót, OBS, wątek terminów
    src/okno.rs            monitory, kotwica okna, skalowanie, odbicie lustrzane
    src/skrot.rs           analiza i zapis kanoniczny skrótu, reguły (AltGr)
    src/dzwiek.rs          dźwięk skupienia: sterownik, urządzenie, rampa, wątek
    src/zasobnik.rs        menu zasobnika i menu pod prawym przyciskiem
    src/system/win.rs      Windows: WTS, zasilanie, GetLastInputInfo, pełny ekran, WDA_EXCLUDEFROMCAPTURE, lista procesów (OBS)
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
- **Monitory:** widget pamięta monitor (nazwa, a gdy nazwy brak, położenie i rozmiar),
  jeden z czterech rogów obszaru roboczego i odstępy w pikselach logicznych. Okno
  rośnie od rogu w stronę środka monitora, odbicie lustrzane zależy od połowy
  monitora, a rozmiar liczy się ze skalowaniem tego monitora. Przesunięcia okna bez
  kliknięcia w postać (system po zmianie monitorów lub skalowania) nie zmieniają
  zapisanej pozycji. Pełny ekran chowa widget tylko wtedy, gdy okno na pierwszym
  planie pokrywa monitor widgetu.
- **Skrót:** domyślnie Ctrl+Alt+Shift+Spacja; stary Ctrl+Alt+Spacja jest zamieniany
  w pliku ustawień raz. Ctrl+Alt z literą jest odrzucany, bo to AltGr (ą, ć, ę…).
  Na czas nagrywania skrótu w ustawieniach własny skrót widgetu jest zwolniony.
- **Kolory** są w `kolory.css`; półprzezroczystości liczy `color-mix()` z tych samych
  zmiennych. Test przechodzi przez arkusze (żaden nie ma koloru wpisanego na sztywno),
  sprawdza kontrasty z B7 i pilnuje, żeby starej palety z A2 nie było też w kodzie TypeScript.
- **Dźwięk gra w Rust.** Wątek `dzwiek` otwiera domyślne wyjście systemu przy pierwszym
  dźwięku w Pracy, zostawia je otwarte 30 s po wyciszeniu (Pauza i Wznów nie budzą
  urządzenia co chwilę), a potem zwalnia. Bufor urządzenia ma 100 ms. Głośność
  zmienia liniowo nakładka przy każdej próbce, od pierwszej odtworzonej, więc nie ma
  schodków ani trzasku po wolnym otwarciu. Wbudowany plik jest rozkodowany raz i
  zapętlony przez `repeat_infinite()` bez przerwy; własny plik gra strumieniowo.
  Chwilowy niedobór danych w buforze (`Xrun`) niczego nie restartuje, a zniknięcie
  urządzenia otwiera je od nowa. Zamknięcie programu wycisza dźwięk w 120 ms.
- **OBS** jest wykrywany po nazwie procesu `obs64.exe` co 2 s i tylko przy włączonej
  ciszy przy nagrywaniu. Suwak na 0 to cisza: sesja nie dostaje znacznika „· 40 Hz”.
