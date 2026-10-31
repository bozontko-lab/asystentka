# Audyt repozytorium `asystentka`

Data audytu: 2026-10-31  
Zakres: zgodność ze `SPEC.md` (wersja 2.1), backend Rust (`src-tauri/`), bezpieczeństwo Tauri, frontend TypeScript/HTML/CSS (`src/`), pokrycie testami, zależności i CI oraz czytelność kodu.

---

## Streszczenie dla osoby nietechnicznej

Projekt jest w bardzo dobrym stanie, działa stabilnie i bezpiecznie wyłącznie na plikach na Twoim dysku, bez łączenia się z internetem. Niemal wszystkie funkcje opisane w projekcie są już gotowe, z wyjątkiem głosu postaci, który zgodnie z planem czeka na dodanie nagrań na samym końcu. Najważniejszym błędem do naprawy jest zachowanie programu tuż po północy: gdy kończysz nocne zadanie, ekran gratulacji i przycisk „Cofnij” znikają po ułamku sekundy, bo program od razu przeskakuje do zamykania minionego dnia. Drugi ważny problem dotyczy przycisku „Cofnij” przy powtarzalnych zadaniach o tej samej nazwie — zamiast cofnąć właśnie zakończone zadanie, program potrafi otworzyć z powrotem stare zadanie sprzed tygodni. Poprawy wymaga też zapisywanie plików w tle, aby notatki dziennika nie traciły znacznika kodowania z Notatnika, a jednoczesna zmiana ustawień z menu i okienka nie powodowała wyścigu na pliku tymczasowym. Przy pracy na dwóch monitorach warto poprawić wykrywanie pełnego ekranu (by postać nie pytała, czy jesteś przy komputerze, gdy oglądasz film na drugim ekranie) oraz przeliczanie wysokości listy zadań po przeniesieniu widgetu. Na koniec warto zatrzymać niewidoczne przełączanie póz, gdy okno jest ukryte, i dopisać kilka automatycznych testów chroniących przed powrotem tych błędów.

---

## Podsumowanie obszarów kontrolnych (1–7)

1. **Zgodność ze `SPEC.md` (v2.1):** Zaimplementowano wszystkie stany, układy, paletę kolorów B7, dźwięk 40 Hz, obsługę wielu monitorów, nagrywanie skrótu i migrację z wersji 1. Sekcja B10 („Głos”) jest celowo niezaimplementowana do czasu pojawienia się plików w `src/assets/glos/` (`SPEC.md:701, 751`). Świadome odejście od wtyczek `plugin-fs` i `plugin-window-state` z B9 (`SPEC.md:664`) na rzecz własnego zapisu atomowego w Ruście i `okno.json` jest udokumentowane w `README.md` i `AGENTS.md`. Wykryto jednak kilka rozbieżności w zachowaniach brzegowych (Z-01 – Z-06).
2. **Rust (bezpieczeństwo pamięci, I/O, wątki, czas):** Kod produkcyjny **nie zawiera niebezpiecznych wywołań `unwrap()` ani `expect()` na ścieżkach i danych użytkownika** — jedyne `expect()` poza `#[cfg(test)]` dotyczą stałych kompilacji (wzorce `Regex` w `OnceLock`, stałe daty/godziny, serializacja domyślnych struktur oraz budowa `tauri::Builder`), a blokady `Mutex` odzyskują stan po zatruciu (`unwrap_or_else(|e| e.into_inner())`). Wykryto natomiast błędy w odznaczaniu duplikatów zadań, zachowaniu BOM w dzienniku oraz kilka wyścigów międzywątkowych (Z-07 – Z-14).
3. **Bezpieczeństwo Tauri:** Brak jakichkolwiek wywołań sieciowych, telemetrii i zewnętrznych API. DOM budowany jest wyłącznie przez `document.createElement` / `textContent` / `append` (`src/dom.ts`), bez użycia `innerHTML` (brak XSS z nazw zadań czy projektów). Wtyczki `opener`, `dialog`, `autostart` i `global-shortcut` nie mają bezpośrednich uprawnień w `capabilities/default.json` — frontend przechodzi wyłącznie przez komendy Rusta. Uwagi dotyczą walidacji ścieżek i drobnego nadmiaru uprawnień/CSP (Z-15 – Z-17).
4. **Frontend TypeScript:** Delegacja zdarzeń w `main.ts` i przebudowa przez `replaceChildren()` zapobiegają wyciekom nasłuchiwaczy DOM. Uwagi dotyczą niezatrzymanych timerów animacji przy ukrytym oknie, drobnego błędu przy anulowaniu zanikania, sprzątania stanu nagrywania skrótu oraz kilku nieobsłużonych obietnic `invoke` (Z-18 – Z-21).
5. **Luki w testach:** Zestaw testów Rusta jest rozbudowany, ale brakuje testów dla kilku kluczowych przecięć warstw oraz jakichkolwiek testów komponentów DOM we frontendzie (Z-22).
6. **Zależności i CI:** Drzewo zależności jest minimalne i spójne z zasadą offline; w workflow GitHub Actions brakuje kilku flag na runnerze Windows oraz ograniczeń uprawnień (Z-23).
7. **Czytelność:** Kod jest konsekwentnie pisany po polsku i dobrze skomentowany, ale trzy pliki Rusta i główny arkusz CSS urosły powyżej 1000–3200 linii, a w stylach i formularzach występują powtórzenia oraz martwe reguły z wersji 1 (Z-24).

---

## Znaleziska szczegółowe

### 1. Zgodność implementacji ze `SPEC.md`

---

#### Z-01. Zakończenie sesji po północy po 20 ms ucina stan `Gratulacje` i odbiera przycisk `Cofnij`

- **Waga:** wysoka
- **Status:** POTWIERDZONE
- **Plik i linia:** `src-tauri/src/rdzen.rs:416-418`, `src-tauri/src/rdzen.rs:746-766`, `src-tauri/src/aplikacja.rs:264`, `src-tauri/src/aplikacja.rs:1080-1083`, `src-tauri/src/aplikacja.rs:1113-1116`
- **Dowód:**
  Według `SPEC.md:492, 495` sesja przechodząca przez północ należy do dnia startu, a po kliknięciu `Zrobione` widget włącza `Gratulacje` z podsumowaniem i przyciskiem `Cofnij` widocznym przez 10 s.
  W kodzie:
  1. Sesja zaczęta np. o `23:30` utrzymuje `self.dz.data` z poprzedniego dnia (`zmiana_dnia` czeka, dopóki `self.dz.sesja.is_some()`).
  2. Gdy użytkownik kliknie `Zrobione` o `00:05`, `zakoncz_sesje` ustawia `self.dz.sesja = None` i `self.faza = Faza::Gratulacje { do_: Some(00:05:10), .. }`, a `Aplikacja::wykonaj` woła `self.obudz()`.
  3. Wątek `monitor` wylicza `najblizszy_termin(00:05:00)`:
     ```rust
     let nastepny_dzien = self.dz.data + Duration::days(1);
     let polnoc = nastepny_dzien.and_hms_opt(0, 0, 0).and_then(|n| lokalna(n, teraz));
     ```
     Ponieważ `polnoc` (`00:00:00`) jest już mniejsza niż `teraz` (`00:05:00`), `najblizszy_termin` zwraca `teraz`.
  4. Po minimalnym uśpieniu pętli (`20 ms`, `aplikacja.rs:1116`) `petla` wywołuje `r.tick(teraz)` → `self.zmiana_dnia(teraz)`. Ponieważ `dzis != self.dz.data` i `self.dz.sesja.is_none()`, `zmiana_dnia` **natychmiast nadpisuje `Faza::Gratulacje`** stanem `KoniecDnia (Wczoraj)` lub `WyborZadan` i resetuje `self.dz`, odbierając użytkownikowi podsumowanie zadania oraz możliwość kliknięcia `Cofnij`.
- **Proponowana poprawka:**
  W `Rdzen::zmiana_dnia` (`rdzen.rs:749`) wstrzymać zmianę dnia także wtedy, gdy trwa `Faza::Gratulacje { do_: Some(_), .. }` (10-sekundowe okno na `Cofnij`), a w `najblizszy_termin` nie zwracać przeterminowanej `polnoc`, dopóki zmiana dnia jest w ten sposób odroczona (zwracać wtedy `do_` z `Faza::Gratulacje`). Dopisać test wywołujący `r.tick` 20 ms po `r.zrobione` po północy.

---

#### Z-02. Pytanie o bezczynność („Jesteś?”) pojawia się mimo pełnego ekranu na drugim monitorze

- **Waga:** średnia
- **Status:** POTWIERDZONE
- **Plik i linia:** `SPEC.md:490`, `src-tauri/src/aplikacja.rs:560-570`, `src-tauri/src/rdzen.rs:371-373`, `src-tauri/src/rdzen.rs:1287-1291`
- **Dowód:**
  `SPEC.md:490` rozróżnia chowanie okna (tylko na monitorze widgetu, B6) od pytania o bezczynność:
  > *„Gdy na którymkolwiek monitorze trwa pełny ekran, nie pyta, bo wtedy możesz oglądać materiał albo grać.”*

  Tymczasem `sprawdz_pelny_ekran` przekazuje do `Rdzen` wyłącznie informację o monitorze widgetu:
  ```rust
  let monitor_widgetu = zablokuj(&self.okno).ekran.as_ref().map(|e| e.pelny);
  let stan = system::pelny_ekran(wlasne);
  let pelny = monitor_widgetu.is_some_and(|m| system::pelny_ekran_na_monitorze(&stan, &m));
  ```
  A `Rdzen::bezczynnosc` sprawdza `self.schowana_przez_pelny_ekran()`:
  ```rust
  if prog == 0 || sekundy < prog || self.stan() != Stan::Praca || self.schowana_przez_pelny_ekran() {
      return;
  }
  ```
  Skutek:
  1. Gdy widget stoi na monitorze bocznym, a użytkownik przez 15 minut ogląda wideo lub gra na padzie na pełnym ekranie na monitorze głównym, widget przechodzi w stan `Jesteś?` (i wycisza dźwięk 40 Hz).
  2. To samo dzieje się na jednym monitorze, jeśli użytkownik wyłączył w ustawieniach `chowaj_przy_pelnym_ekranie` albo ręcznie odkrył widget skrótem klawiszowym.
- **Proponowana poprawka:**
  W `system` dodać funkcję `pelny_ekran_gdziekolwiek(stan: &StanPelnegoEkranu) -> bool` (np. `stan.pierwszy_plan.is_some_and(|p| p.pokrywa_monitor && (!p.zwykle_okno || stan.sygnal_systemu))`), przekazywać tę flagę do `Rdzen` niezależnie od `pelny_ekran_na_monitorze` i w `Rdzen::bezczynnosc` blokować pytanie `Jesteś?`, gdy `self.pelny_ekran_gdziekolwiek` jest `true`.

---

#### Z-03. Zbliżenie kursora (<300 px) w stanie `Praca` nadpisuje pozę konsolki oraz kubka i wyłącza mruganie

- **Waga:** średnia
- **Status:** POTWIERDZONE
- **Plik i linia:** `SPEC.md:538-541`, `src/postac.ts:251-256`
- **Dowód:**
  `SPEC.md:541` definiuje spojrzenie jako wariant pozy obserwowania pracy:
  > *„`sit_watch_front`, gdy kursor jest bliżej niż 300 px od widgetu, w przeciwnym razie `sit_watch`; sprawdzane co 500 ms”*

  W `src/postac.ts:251-256` warunek `this.blisko` jest sprawdzany przed wariantami `konsola` i `kubek`:
  ```ts
  case "praca":
    if (this.przeciaga) return { a: "pause_stretch", tryb: "brak" };
    if (this.blisko && g.ma("sit_watch_front")) return { a: "sit_watch_front", tryb: "brak" };
    if (this.wariant === "konsola") return this.konsolka();
    if (this.wariant === "kubek") return { a: "sit_drink", tryb: "brak" };
    return { a: "sit_watch", b: "sit_watch_blink", tryb: "mrug" };
  ```
  Gdy postać wylosuje `sit_drink` lub `sit_console_1`, każde zbliżenie kursora na odległość <300 px powoduje nagłe „zniknięcie” kubka lub konsolki na rzecz `sit_watch_front`, a po odsunięciu kursora rekwizyt wraca. Jeśli kursor stale spoczywa w tym rogu ekranu, warianty `konsola` i `kubek` nigdy się nie pokażą, a postać nigdy nie mruga (`tryb: "brak"`).
- **Proponowana poprawka:**
  Sprawdzać `this.blisko` wyłącznie wtedy, gdy `this.wariant === "patrzy"` (tj. przenieść `if (this.blisko && g.ma("sit_watch_front"))` poniżej warunków `konsola` i `kubek`).

---

#### Z-04. Limit wysokości panelu (60% monitora) jest wyliczany z `window.screen.availHeight` przed przeniesieniem okna na monitor docelowy

- **Waga:** średnia
- **Status:** POTWIERDZONE
- **Plik i linia:** `SPEC.md:511-514`, `src/main.ts:54-57`, `src/main.ts:159`, `src/main.ts:293-298`, `src-tauri/src/aplikacja.rs:292-303`
- **Dowód:**
  `SPEC.md:511-514` wymaga, by rozmiar panelu wyboru (do 60% wysokości monitora) liczyć zawsze względem obszaru roboczego monitora widgetu, nigdy monitora głównego.
  W `src/main.ts:55-57`:
  ```ts
  function ustawMaksWysokosc(): void {
    document.documentElement.style.setProperty("--panel-max-h", `${Math.round(window.screen.availHeight * 0.6)}px`);
  }
  ```
  1. Przy starcie okno ma `visible: false` i zanim frontend wywoła `await most.gotowy(szer, wys)` (`main.ts:298`), znajduje się na domyślnym monitorze głównym. Wywołanie `rysuj(await most.migawka())` (`main.ts:293`) liczy więc `--panel-max-h` dla monitora głównego.
  2. Dopiero `gotowy()` w Ruście (`aplikacja.rs:294`) przenosi okno przez `umiesc_okno()` na drugi monitor z `okno.json` (np. mniejszy ekran laptopa albo pionowy monitor). Jeśli narożnik (`PrawyDolny`) jest taki sam jak domyślny, backend nie wysyła nowej migawki, więc `--panel-max-h` zostaje z wysokością monitora głównego.
  3. Również po ręcznym przeciągnięciu widgetu na drugi monitor o tym samym narożniku brak zdarzenia, które przeliczyłoby `--panel-max-h` i zgłosiło nowy rozmiar okna.
- **Proponowana poprawka:**
  Przekazywać logiczną wysokość obszaru roboczego bieżącego monitora widgetu (`obszar.wys / obszar.skala`) z backendu w strukturze `Migawka` (i aktualizować w `umiesc_okno()`, gdy zmieni się `stan.ekran`) zamiast polegać na `window.screen.availHeight`.

---

#### Z-05. Tytuł otwartego okna ustawień nie aktualizuje się od razu po zmianie `nazwa_postaci`

- **Waga:** niska
- **Status:** POTWIERDZONE
- **Plik i linia:** `SPEC.md:520, 739`, `src-tauri/src/aplikacja.rs:584-595`, `src-tauri/src/aplikacja.rs:919-931`, `src-tauri/src/aplikacja.rs:979-989`
- **Dowód:**
  Kryterium akceptacji w `SPEC.md:739` brzmi:
  > *„Po zmianie `nazwa_postaci` nowe imię od razu widać w dymkach, obu menu, podpowiedzi zasobnika i tytule okienka ustawień.”*

  Gdy okno ustawień jest otwarte, a użytkownik zmieni `nazwa_postaci` bezpośrednio w `ustawienia.json` (wykryte przez `sprawdz_ustawienia` w `aplikacja.rs:584`) albo wywoła `otworz_okno_ustawien` ponownie dla już istniejącego okna (`aplikacja.rs:984-989`):
  ```rust
  let tytul = format!("{} · ustawienia", zablokuj(&self.rdzen).ust.postac());
  thread::spawn(move || {
      if let Some(okno) = app.get_webview_window(OKNO_USTAWIEN) {
          let _ = okno.show();
          let _ = okno.unminimize();
          let _ = okno.set_focus();
          return;
      }
  ```
  Obliczony `tytul` jest ignorowany dla istniejącego okna (brak `okno.set_title(&tytul)`), a `sprawdz_ustawienia` i `zapisz_ustawienia` również nie aktualizują tytułu otwartego `OKNO_USTAWIEN`.
- **Proponowana poprawka:**
  Dodać `let _ = okno.set_title(&tytul);` w gałęzi istniejącego okna w `otworz_okno_ustawien` oraz aktualizować tytuł `OKNO_USTAWIEN` (jeśli istnieje) przy wczytaniu/zapisaniu ustawień.

---

#### Z-06. `CloseRequested` (np. `Alt+F4`) na oknie widgetu przełącza widoczność zamiast chować i nie cofa `focusable`

- **Waga:** niska
- **Status:** POTWIERDZONE
- **Plik i linia:** `src-tauri/src/lib.rs:129-135`, `src/main.ts:83-87`, `src/main.ts:189`
- **Dowód:**
  Komentarz w `src-tauri/src/lib.rs:129-135` deklaruje chowanie okna:
  ```rust
  // Widget się nie zamyka (np. Alt+F4), tylko chowa; wyjście jest w zasobniku.
  WindowEvent::CloseRequested { api, .. } if okno.label() == aplikacja::OKNO => {
      api.prevent_close();
      if let Some(apl) = apl(okno.app_handle()) {
          apl.wykonaj(|r, _| r.przelacz_widocznosc());
      }
  }
  ```
  1. `Alt+F4` może trafić do `OKNO` właśnie wtedy, gdy okno ma włączony fokus (`focusable: true` podczas otwartego formularza „Dodaj zadanie” lub kreatora). Po schowaniu okna formularz `dodawanie` we frontendzie nadal istnieje (`dodawanie !== null`), więc `ustawFokus(...)` w `main.ts:189` zostawia `focusable: true`.
  2. Użyto `przelacz_widocznosc()` zamiast bezwarunkowego ukrycia (`ukryj()`), więc wysłanie sygnału zamknięcia do ukrytego okna powoduje jego pokazanie.
- **Proponowana poprawka:**
  Dodać w `Rdzen` metodę `ukryj_recznie()` (`self.ukrycie.recznie = true`), wywoływać ją w `CloseRequested` zamiast `przelacz_widocznosc()`, a w `main.ts:189` uzależnić fokus od widoczności: `ustawFokus(m.widoczna && (m.stan === "konfiguracja" || dodawanie !== null))`.

---

### 2. Rust: błędy odczytu/zapisu, zapis atomowy, kolejka ponowień, wyścigi, północ i zmiana czasu

---

#### Z-07. Przycisk `Cofnij` (`odznacz_zrobione`) przy zduplikowanych nazwach zadań otwiera stare, historyczne zadanie

- **Waga:** wysoka
- **Status:** POTWIERDZONE
- **Plik i linia:** `src-tauri/src/plik_zadan.rs:65-70`, `src-tauri/src/plik_zadan.rs:279-319`
- **Dowód:**
  W jednym projekcie może znajdować się wcześniejsze ukończone zadanie o tej samej nazwie oraz nowe otwarte zadanie (scenariusz wprost wspierany i testowany w `plik_zadan.rs:562`, test `duplikaty_najpierw_otwarte`):
  ```markdown
  ## Inbox
  - [x] Przegląd maili [czas:: 15] ✅ 2026-10-01
  - [ ] Przegląd maili [szac:: 15]
  ```
  1. Gdy użytkownik kończy dzisiejsze zadanie przez `Zrobione`, `dopisz_czas` używa `PlikZadan::zadanie`, które szuka pierwszego **otwartego** zadania (`!z.zrobione`), więc poprawnie oznacza jako zrobione **drugą** linię (`✅ 2026-10-31`). W pliku obie linie mają teraz `[x]`.
  2. Gdy użytkownik w ciągu 10 s kliknie `Cofnij`, `odznacz_zrobione` wykonuje:
     ```rust
     let cel = plik
         .zadania
         .iter()
         .find(|z| &z.klucz == klucz && z.zrobione)
         .or_else(|| plik.zadania.iter().find(|z| &z.klucz == klucz))
         .ok_or(BladEdycji::NieZnaleziono)?
         .linia;
     ```
     `iter().find(...)` znajduje **pierwszą** linię od góry — czyli historyczne zadanie z `2026-10-01`! Usuwa z niego `[x]` oraz historyczną datę `✅ 2026-10-01`, natomiast dzisiejsze zadanie zostawia zamknięte z `[x]`, a wznowiona sesja zaczyna dopisywać czas do starego wpisu.
- **Proponowana poprawka:**
  W `odznacz_zrobione` przekazywać datę ukończenia (lub preferować zadanie z dzisiejszą datą `✅ RRRR-MM-DD`, a w drugiej kolejności szukać od końca sekcji `.rev()`, ponieważ nowe powtarzalne zadania są dopisywane poniżej ukończonych — por. `dopisz_zadanie` w `plik_zadan.rs:362`).

---

#### Z-08. `dziennik::zapisz` usuwa znacznik UTF-8 BOM z istniejących notatek dziennika

- **Waga:** średnia
- **Status:** POTWIERDZONE
- **Plik i linia:** `src-tauri/src/pliki.rs:47-53`, `src-tauri/src/dziennik.rs:141-145`, `src-tauri/src/dziennik.rs:181-183`, `src-tauri/src/dziennik.rs:203-212`
- **Dowód:**
  Zasady architektury (`AGENTS.md`, `SPEC.md:389`) wymagają zachowania BOM i CRLF w edytowanych plikach użytkownika. Funkcja `dziennik::aktualizuj_tresc` została napisana z myślą o zachowaniu BOM:
  ```rust
  let bom = stara.starts_with('\u{feff}');
  ...
  if bom {
      wynik.insert(0, '\u{feff}');
  }
  ```
  Jednak `dziennik::zapisz` wczytuje plik przez `pliki::czytaj_tekst(&plik)` (`dziennik.rs:203`), które **bezwarunkowo usuwa** `\u{feff}` przed zwróceniem tekstu (`pliki.rs:49-52`):
  ```rust
  let stara = match pliki::czytaj_tekst(&plik) {
      Ok(t) => Some(t),
      ...
  };
  let nowa = aktualizuj_tresc(stara.as_deref(), dzien);
  ```
  W rezultacie `stara` przekazana do `aktualizuj_tresc` nigdy nie zaczyna się od `\u{feff}`, `bom` wynosi zawsze `false`, a zapis na dysk gubi BOM istniejącej notatki dziennika.
- **Proponowana poprawka:**
  W `dziennik::zapisz` odczytywać plik z zachowaniem informacji o BOM (np. przez `fs::read_to_string(&plik)` bez zdejmowania `\u{feff}`, tak aby `stara` zawierała oryginalny prefiks `\u{feff}` — wtedy zarówno `aktualizuj_tresc`, jak i porównanie `if stara.as_deref() == Some(nowa.as_str())` w linii 209 zadziałają poprawnie).

---

#### Z-09. Wyścig odczyt–modyfikacja–zapis oraz kolizja nazwy pliku tymczasowego przy zapisie `ustawienia.json`

- **Waga:** średnia
- **Status:** POTWIERDZONE
- **Plik i linia:** `src-tauri/src/pliki.rs:24-44`, `src-tauri/src/aplikacja.rs:800-814`, `src-tauri/src/aplikacja.rs:831-845`, `src-tauri/src/aplikacja.rs:910-912`
- **Dowód:**
  1. W `pliki::zapisz_atomowo` nazwa pliku tymczasowego zależy wyłącznie od PID procesu:
     ```rust
     let tymczasowy = folder.join(format!(".{nazwa}.{}.tmp", std::process::id()));
     ```
  2. Trzy ścieżki w `Aplikacja` zapisują `ustawienia.json` **bez żadnej wspólnej blokady** (poza muteksem `self.rdzen`):
     - `przelacz_ukrycie_przed_nagrywaniem` (`aplikacja.rs:806`, wątek zdarzeń zasobnika),
     - `przelacz_dzwiek` (`aplikacja.rs:836`, wątek `async` komendy lub wątek zasobnika),
     - `zapisz_i_zastosuj` (`aplikacja.rs:911`, wątek `async` komendy `ustawienia_zapisz`).
  3. Gdy użytkownik kliknie „Zapisz” w otwartym oknie ustawień równocześnie z przełączeniem dźwięku lub ukrycia w zasobniku/panelu:
     - oba wątki wykonują równolegle `wczytaj_dokument` → modyfikacja w pamięci → `zapisz_atomowo`, co prowadzi do klasycznego zgubienia zmiany (lost update),
     - oba wątki jednocześnie piszą i wywołują `fs::rename` / `fs::remove_file` na **dokładnie tej samej ścieżce** `.ustawienia.json.<pid>.tmp`, co na Windows kończy się błędem dostępu lub usunięciem pliku tymczasowego drugiego wątku.
- **Proponowana poprawka:**
  1. W `pliki::zapisz_atomowo` dodać globalny `AtomicU64` do nazwy pliku tymczasowego: `.{nazwa}.{pid}.{nr}.tmp`.
  2. W `Aplikacja` objąć wszystkie operacje zapisu `ustawienia.json` (wraz z aktualizacją `self.ustawienia_zmienione`) dedykowanym muteksem `zapis_ustawien: Mutex<()>`.

---

#### Z-10. Wyścig w `Aplikacja::wykonaj` może nadpisać w UI, zasobniku i silniku audio nowszy stan starszą migawką

- **Waga:** średnia
- **Status:** POTWIERDZONE
- **Plik i linia:** `src-tauri/src/aplikacja.rs:242-280`, `src-tauri/src/aplikacja.rs:419-428`
- **Dowód:**
  `Aplikacja::wykonaj` słusznie zwalnia blokadę `self.rdzen` przed wywołaniami Tauri i audio (aby uniknąć zakleszczenia z wątkiem głównym), ale nie przypisuje migawce numeru sekwencyjnego pod blokadą `self.rdzen`:
  ```rust
  let (wynik, migawka, zdarzenia, menu, ochrona, dzwiek) = {
      let mut r = zablokuj(&self.rdzen);
      let wynik = dzialanie(&mut r, teraz);
      ...
      (wynik, r.migawka(), zdarzenia, stan_menu(&r), r.ust.ukryj_przed_nagrywaniem, r.dzwiek_cel())
  }; // <-- zwolnienie locka self.rdzen
  if let Some(silnik) = self.dzwiek.get() {
      silnik.cel(dzwiek);
  }
  self.rozeslij(&migawka, zdarzenia);
  if self.gotowy.load(Ordering::SeqCst) {
      self.zastosuj_widocznosc(migawka.widoczna);
  }
  if let Some(zasobnik) = self.zasobnik.get() {
      zasobnik.odswiez(&menu);
  }
  ```
  Scenariusz: wątek `monitor` wykonuje `tick`/`cykl` (stan `S1`), zwalnia `self.rdzen` i zostaje wywłaszczony. W tym momencie wątek `async` komendy wykonuje kliknięcie użytkownika (np. `pauza` lub `rozpocznij` → stan `S2`), zwalnia `self.rdzen`, ustawia `silnik.cel(dzwiek_2)` i wysyła `migawka_2`. Wznowiony wątek `monitor` wykonuje teraz swoje wywołania dla `S1`: ponieważ `json_1 != json_2`, `rozeslij` nadpisuje `ostatnia_migawka` i emituje do frontendu **przestarzałą `migawka_1`**, a `silnik.cel(dzwiek_1)` i `zasobnik.odswiez(&menu_1)` cofają dźwięk oraz menu do stanu `S1`.
- **Proponowana poprawka:**
  Dodać w `Rdzen` (lub pod blokadą `self.rdzen`) rosnący licznik `wersja: u64`, zwracać go razem z `migawka`, a rozsyłanie skutków (`silnik.cel`, `rozeslij`, `zastosuj_widocznosc`, `zasobnik.odswiez`, `zastosuj_ochrone`) serializować lekkim muteksem `rozsylanie: Mutex<u64>`, pomijając aktualizację migawki/celu/widoczności/menu, jeśli `wersja < *ostatnia_wersja` (same `zdarzenia` z `wez_zdarzenia()` należy wyemitować zawsze, zanim wyemituje się nowszą migawkę, lub emitować w kolejności wersji).

---

#### Z-11. Wyścig na `plik_zmieniony` między wątkiem `notify` a wątkiem `monitor` może zgubić ostatni zapis `Zadania.md`

- **Waga:** średnia
- **Status:** POTWIERDZONE
- **Plik i linia:** `src-tauri/src/aplikacja.rs:631-634`, `src-tauri/src/aplikacja.rs:1092-1102`, `src-tauri/src/rdzen.rs:730-732`
- **Dowód:**
  W `petla()` wątek `monitor` odczytuje i czyści `self.plik_zmieniony` w dwóch osobnych sekcjach krytycznych:
  ```rust
  let zmiana_pliku = *zablokuj(&self.plik_zmieniony);
  if let Some(od) = zmiana_pliku {
      if od.elapsed() >= ODSTEP_PO_ZAPISIE {
          *zablokuj(&self.plik_zmieniony) = None;
          self.wykonaj(|r, _| {
              r.odswiez_zadania();
          });
  ```
  Jeśli po odczycie `zmiana_pliku` (gdy `od.elapsed() >= 300 ms`), a przed wpisaniem `None` (lub w trakcie zapisu pliku przez edytor wieloetapowo), wątek `notify` zapisze świeży znacznik `*zablokuj(&apl.plik_zmieniony) = Some(Instant::now())`, linia `1095` bezwarunkowo nadpisze go wartością `None`. Jeżeli `odswiez_zadania()` trafi na moment przed zakończeniem drugiego zapisu edytora, a `self.obserwacja_pliku == true` wyłącza 30-sekundowe odpytywanie w `Rdzen::cykl` (`rdzen.rs:730`), lista zadań w widgecie pozostanie nieaktualna aż do kolejnego zapisu w pliku.
- **Proponowana poprawka:**
  Sprawdzać i zerować `plik_zmieniony` atomowo pod jedną blokadą muteksu (warunkowo: zerować tylko wtedy, gdy aktualna wartość nadal spełnia `t.elapsed() >= ODSTEP_PO_ZAPISIE`, w przeciwnym razie wyznaczyć nowy `nastepny = nastepny.min(t + ODSTEP_PO_ZAPISIE)`).

---

#### Z-12. Korekta zegara systemowego (>90 s) pauzuje sesję jak po uśpieniu, a zmiana czasu (DST) przesuwa wyliczony termin o godzinę do momentu zmiany offsetu

- **Waga:** niska
- **Status:** POTWIERDZONE
- **Plik i linia:** `src-tauri/src/aplikacja.rs:1058-1067`, `src-tauri/src/rdzen.rs:1683-1685`
- **Dowód:**
  1. W `aplikacja.rs:1059` wykrywanie uśpienia bez komunikatu z Windows opiera się wyłącznie na zegarze ściennym (`czas::teraz()`):
     ```rust
     if teraz - ostatni_obrot > chrono::Duration::seconds(90) {
         ...
         self.wykonaj(|r, t| {
             r.pauza_automatyczna(PowodPauzy::Uspienie, od);
             r.powrot(t);
         });
     }
     ```
     Gdy system przestawi zegar o >90 s do przodu (np. synchronizacja NTP po starcie lub ręczna korekta godziny), aktywna sesja zostaje automatycznie zapauzowana z dymkiem „Wracamy?”, mimo że komputer nie był uśpiony. Podobnie wiosenna zmiana czasu na letni (`02:00 +01:00` → `03:00 +02:00`) jest poprawnie neutralna w `chrono::DateTime<FixedOffset>` (różnica UTC to nadal ≤30 s), ale każda korekta zegara UTC >90 s wyzwala fałszywe uśpienie.
  2. W `rdzen.rs:1683-1685` funkcja `lokalna` wyznacza termin (np. `22:00` lub `00:00`) przy użyciu offsetu z chwili `wzor` (`*wzor.offset()`):
     ```rust
     fn lokalna(naiwna: chrono::NaiveDateTime, wzor: Chwila) -> Option<Chwila> {
         naiwna.and_local_timezone(*wzor.offset()).single()
     }
     ```
     W dniu zmiany czasu (np. o `01:30` przed zmianą offsetu o `02:00`/`03:00`) termin `22:00` jest wyliczany jeszcze ze starym offsetem. Ponieważ pętla `monitor` budzi się co najwyżej co 30 s (`CO_ILE_CYKL`), po przekroczeniu godziny zmiany czasu `czas::teraz()` otrzymuje nowy offset i termin koryguje się sam, jednak w `lokalna` czystsze jest użycie `chrono::Local` (z wyborem `.earliest()`/`.latest()` przy niejednoznacznej godzinie jesiennej), o ile chcemy zachować brak zależności `rdzen.rs` od systemu lub udostępnić konwersję w `czas.rs`.
- **Proponowana poprawka:**
  W `czas.rs` oprzeć konwersję lokalnej daty/godziny na `chrono::Local` (`and_local_timezone(Local).earliest()`), a w `aplikacja.rs` logować korektę zegara odróżniając ją (jeśli możliwe na danej platformie) od rzeczywistego zawieszenia pętli.

---

#### Z-13. `Ordering::Relaxed` w `dzwiek::Sterowanie` nie gwarantuje porządku zapisu i odczytu dwóch atomików deklarowanego w komentarzu

- **Waga:** niska
- **Status:** POTWIERDZONE
- **Plik i linia:** `src-tauri/src/dzwiek.rs:425-445`, `src-tauri/src/dzwiek.rs:470-471`
- **Dowód:**
  W `dzwiek.rs:431-445`:
  ```rust
  impl Sterowanie {
      fn ustaw(&self, cel: f32, nachylenie: f32) {
          // Najpierw tempo, potem cel: nowy cel nigdy nie trafia na stare, zerowe tempo.
          self.nachylenie.store(nachylenie.to_bits(), Ordering::Relaxed);
          self.cel.store(cel.to_bits(), Ordering::Relaxed);
      }

      fn cel(&self) -> f32 {
          f32::from_bits(self.cel.load(Ordering::Relaxed))
      }

      fn nachylenie(&self) -> f32 {
          f32::from_bits(self.nachylenie.load(Ordering::Relaxed))
      }
  }
  ```
  W modelu pamięci Rusta operacje `Ordering::Relaxed` na dwóch różnych zmiennych `AtomicU32` nie tworzą relacji *happens-before*; optymalizator kompilatora może zmienić kolejność instrukcji zapisu lub odczytu, przez co wątek audio w `Rampa::next` (`dzwiek.rs:470-471`) może zobaczyć nowy `cel` ze starym `nachylenie` (np. `0.0`), wbrew komentarzowi w linii 433.
- **Proponowana poprawka:**
  Użyć `Ordering::Release` przy `self.cel.store(...)` oraz `Ordering::Acquire` przy `self.cel.load(...)` (lub zapisać parę `(cel, nachylenie)` atomowo w pojedynczym `AtomicU64`).

---

#### Z-14. `pokaz_bez_aktywacji` używa `okno.show()` i `SetForegroundWindow`, co może na ułamek sekundy odebrać fokus innej aplikacji

- **Waga:** niska
- **Status:** HIPOTEZA
- **Plik i linia:** `src-tauri/src/system/win.rs:233-246`
- **Dowód:**
  ```rust
  pub fn pokaz_bez_aktywacji(okno: &WebviewWindow) {
      let poprzednie = unsafe { GetForegroundWindow() };
      let _ = okno.show();
      let Some(nasze) = hwnd(okno) else { return };
      unsafe {
          if GetForegroundWindow() == nasze ... {
              let _ = SetForegroundWindow(poprzednie);
          }
      }
  }
  ```
  Chociaż okno ma w konfiguracji Tauri `focusable: false` (`WS_EX_NOACTIVATE`), wywołanie `okno.show()` przechodzi przez ogólną warstwę `tao`/`wry`. Gdyby okno było w tym momencie `focusable: true` (np. schowane skrótem w trakcie otwartego formularza „Dodaj zadanie” albo kreatora, a potem odkryte skrótem), `okno.show()` aktywuje widget i wyśle `WM_ACTIVATE(WA_INACTIVE)` do poprzedniego okna zanim `SetForegroundWindow(poprzednie)` odda fokus — co w grach Exclusive Fullscreen na drugim monitorze lub w otwartych menu kontekstowych innych programów potrafi zamknąć menu lub zminimalizować grę. Wymaga sprawdzenia na żywym Windows 10/11.
- **Proponowana poprawka:**
  Pokazywać okno bezpośrednio przez Win32 `ShowWindow(uchwyt, SW_SHOWNA)` (lub `SetWindowPos` z `SWP_SHOWWINDOW | SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER`), które z definicji nie aktywuje okna na poziomie systemu.

---

### 3. Bezpieczeństwo Tauri: capabilities, CSP, ścieżki użytkownika i komendy backendu

---

#### Z-15. Brak weryfikacji rozszerzenia `.md` dla `plik_zadan` w połączeniu z automatycznym `opener().open_path(...)`

- **Waga:** średnia
- **Status:** POTWIERDZONE
- **Plik i linia:** `src-tauri/src/ustawienia.rs:121-157`, `src-tauri/src/aplikacja.rs:891-909`, `src-tauri/src/aplikacja.rs:933-962`
- **Dowód:**
  1. `Ustawienia::waliduj()` (`ustawienia.rs:123-128`) sprawdza jedynie, czy `plik_zadan()` nie jest pusty i nie zawiera `<` ani `>`. Nie wymusza rozszerzenia `.md` / `.markdown`.
  2. Komenda `kreator_zakoncz(ustawienia, utworz_plik)` wywołuje `zakoncz_kreator` (`aplikacja.rs:933-948`):
     ```rust
     if let (true, Some(plik)) = (utworz_plik, plik) {
         self.otworz_plik(&plik);
     }
     ```
     gdzie `otworz_plik` poza vaultem Obsidiana wywołuje `self.app.opener().open_path(plik.to_string_lossy(), None::<&str>)` (`ShellExecuteW` na Windows).
     Gdyby wywołano `kreator_zakoncz` ze ścieżką spoza vaulta zakończoną rozszerzeniem wykonywalnym/skryptowym (np. `.bat`, `.cmd`), backend najpierw utworzy ten plik, a następnie uruchomi go w systemie przez `open_path`.
  3. Podobnie w `ustawienia_zapisz` brak wymogu rozszerzenia `.md` pozwala wskazać dowolny istniejący plik tekstowy UTF-8 i dopisać do niego linie Markdown komendą `dodaj_zadanie`, a `folder_dziennika` tworzy dowolną ścieżkę przez `fs::create_dir_all`.
- **Proponowana poprawka:**
  W `Ustawienia::waliduj()` wymagać, aby `plik_zadan` miał rozszerzenie `.md` lub `.markdown` (bez rozróżniania wielkości liter), a przed `opener().open_path(plik, None)` dodatkowo asercyjnie sprawdzać `matches!(plik.extension().and_then(|e| e.to_str()), Some(e) if e.eq_ignore_ascii_case("md") || e.eq_ignore_ascii_case("markdown"))`.

---

#### Z-16. Nieużywane uprawnienie `allow-start-dragging`, współdzielony profil capabilities dla obu okien oraz nierozwijanie `%ZMIENNA%` w `plik_istnieje`

- **Waga:** niska
- **Status:** POTWIERDZONE
- **Plik i linia:** `src-tauri/capabilities/default.json:5-14`, `src-tauri/src/komendy.rs:176-193`, `src-tauri/src/ustawienia.rs:163-196`
- **Dowód:**
  1. W `src-tauri/capabilities/default.json`:
     ```json
     "windows": ["main", "ustawienia"],
     "permissions": [
       "core:default",
       "core:window:allow-start-dragging",
       "core:window:allow-close",
       "log:allow-log"
     ]
     ```
     - `core:window:allow-start-dragging` nie jest nigdzie używane we frontendzie (przeciąganie realizuje komenda Rust `przeciagnij` w `komendy.rs:38`), a mimo to jest nadane obu oknom (także `ustawienia`).
     - `core:window:allow-close` jest używane wyłącznie przez okno `ustawienia` (`most.ts:102`).
  2. Komenda `plik_istnieje` (`komendy.rs:176-178`) oraz pomocnik `katalog_startowy` (`komendy.rs:186-193`) sprawdzają surowy napis `Path::new(sciezka.trim())`, podczas gdy `Ustawienia::plik_zadan()` (`ustawienia.rs:168`) rozwija zmienne `%USERPROFILE%` i `~/`. W rezultacie wpisanie w kreatorze ścieżki `%USERPROFILE%\Vault\Zadania.md` do istniejącego pliku sprawi, że `plik_istnieje` zwróci `false`: w trybie „Mam własny plik” kreator zablokuje przejście błędem „Nie widzę tego pliku”, a w trybie „Załóż nowy plik” wyśle `utworzPlik: true` i odbije się w backendzie od `AlreadyExists`.
- **Proponowana poprawka:**
  Usunąć `core:window:allow-start-dragging` z `capabilities/default.json` i wydzielić `core:window:allow-close` tylko dla okna `"ustawienia"`. W `komendy::plik_istnieje` i `katalog_startowy` przepuszczać ścieżkę przez wspólną funkcję rozwijania zmiennych z `ustawienia.rs`.

---

#### Z-17. Dyrektywa `'unsafe-inline'` w `style-src` CSP jest potrzebna tylko przez jeden atrybut `style` w `widok.ts` i zmienne CSS

- **Waga:** niska
- **Status:** POTWIERDZONE
- **Plik i linia:** `src-tauri/tauri.conf.json:33`, `src/widok.ts:250`
- **Dowód:**
  W `src-tauri/tauri.conf.json:33`:
  ```json
  "csp": "default-src 'self'; img-src 'self' data: blob:; style-src 'self' 'unsafe-inline'; script-src 'self'"
  ```
  `style-src 'unsafe-inline'` jest wymagane przez bezpośrednie ustawianie atrybutu HTML `style` w `src/widok.ts:250`:
  ```ts
  el("div", { class: "pasek" }, el("div", { class: "pasek-wypelnienie", style: `transform: scaleX(${udzial.toFixed(3)})` }))
  ```
  (oraz manipulacje CSSOM `.style.setProperty` / `.style.transform` w `main.ts`, `efekty.ts`, `postac.ts`, `widok.ts` — przy czym w Chromium/WebView2 atrybut tekstowy `setAttribute("style", ...)` w `el()` podlega blokadzie CSP, podczas gdy dostęp przez API CSSOM często zależy od wersji silnika).
- **Proponowana poprawka:**
  W `widok.ts:250` ustawiać `element.style.transform = ...` przez właściwości obiektu zamiast `setAttribute("style", ...)` w `el()` (dla spójności z `widok.ts:322`); jeśli WebView2 nadal wymaga `'unsafe-inline'` dla `.style.setProperty`, pozostawić z komentarzem wyjaśniającym powód.

---

### 4. Frontend TypeScript: timery, nasłuchiwacze i obsługa błędów z `invoke`

---

#### Z-18. Ukryte okno widgetu nie zatrzymuje timerów JS w `Postac`, a dwa `setTimeout` nie mają uchwytów do anulowania

- **Waga:** średnia
- **Status:** POTWIERDZONE
- **Plik i linia:** `SPEC.md:525`, `src/postac.ts:90-99`, `src/postac.ts:108-116`, `src/postac.ts:154-214`, `src/postac.ts:325-335`
- **Dowód:**
  Gdy okno zostaje ukryte (np. pełny ekran w trakcie gry lub skrót klawiszowy), `main.ts:138` wywołuje `postac.widocznosc(false)`. Metoda ta zatrzymuje **wyłącznie** `zegarMrugania`:
  ```ts
  widocznosc(widoczna: boolean): void {
    if (this.widoczna === widoczna) return;
    this.widoczna = widoczna;
    if (widoczna) {
      this.zaplanujMrugniecie();
    } else {
      window.clearTimeout(this.zegarMrugania);
      this.cialo.classList.remove("mrug");
    }
  }
  ```
  1. W trakcie wielogodzinnej gry na pełnym ekranie przy aktywnej sesji (`praca` lub `pauza`) w ukrytym oknie nadal cyklicznie odpalają się timery JavaScript:
     - `zegarWariantu` (`zaplanujZmianePozy`, co 3–6 min),
     - `zegarKonsoli` (`zaplanujKonsole`, co 2 min i po 3 s),
     - `zegarFazy` (`zaplanujFazePrzerwy`),
     każdorazowo wywołując `odswiez(true)` → przebudzenia JS, przełączanie klas `.zanik` z kolejnym `setTimeout(..., 75)` oraz podmianę `HTMLImageElement.src` w ukrytym WebView2.
  2. Dodatkowo dwa wywołania `window.setTimeout` nie zapisują swojego identyfikatora i nigdy nie są anulowane przy zmianie stanu lub ukryciu okna:
     - `przeciagnijSie()` (`postac.ts:112`, timer 4000 ms) — szybkie wyjście ze stanu `praca` i powrót do niego w ciągu 4 s spowoduje nieoczekiwane `odswiez(true)`,
     - wewnętrzny timer końca mrugnięcia (`postac.ts:330`, timer 130 ms).
- **Proponowana poprawka:**
  W `widocznosc(false)` czyścić wszystkie aktywne timery klasy `Postac` (`zegarMrugania`, wewnętrzny `zegarKoncaMrugania`, `zegarWariantu`, `zegarFazy`, `zegarKonsoli`, `zegarStanu` oraz nowy uchwyt `zegarPrzeciagania`), a w `widocznosc(true)` planować je ponownie na podstawie aktualnego stanu i `Date.now()` oraz wywołać `odswiez(false)`.

---

#### Z-19. Przedwczesne zdjęcie klasy `.zanik` w unieważnionym callbacku zanikania powoduje mignięcie starej pozy

- **Waga:** niska
- **Status:** POTWIERDZONE
- **Plik i linia:** `src/postac.ts:313-319`
- **Dowód:**
  ```ts
  if (zanik && !pierwszy) {
    this.cialo.classList.add("zanik");
    window.setTimeout(() => {
      if (numer === this.numerZmiany) zastosuj();
      else this.cialo.classList.remove("zanik");
    }, ZANIK_MS);
  }
  ```
  Gdy w `t = 0 ms` startuje zmiana pozy nr 1 z zanikaniem (`ZANIK_MS = 75 ms`), a w `t = 40 ms` startuje zmiana pozy nr 2 (również z `zanik = true`), w `t = 75 ms` odpala przestarzały callback nr 1. Ponieważ `1 !== 2`, wykonuje `else this.cialo.classList.remove("zanik")`, co rozjaśnia postać ze **starą** grafiką, a w `t = 115 ms` callback nr 2 podmienia `obrazA.src` na pełnej widoczności bez wygaszenia.
- **Proponowana poprawka:**
  Przechowywać identyfikator `zegarZaniku` i anulować go przez `window.clearTimeout(this.zegarZaniku)` przed zaplanowaniem kolejnego przejścia (lub po prostu usunąć gałąź `else this.cialo.classList.remove("zanik")`, ponieważ nowsza zmiana zawsze sama zdejmie `.zanik` w swoim `zastosuj()`).

---

#### Z-20. Zamknięcie okna ustawień podczas nagrywania skrótu może pozostawić wyłączony globalny skrót klawiszowy

- **Waga:** niska
- **Status:** HIPOTEZA
- **Plik i linia:** `src/skrot.ts:104-105`, `src/skrot.ts:128`, `src/skrot.ts:137`, `src-tauri/src/lib.rs:123-137`
- **Dowód:**
  Podczas nagrywania kombinacji `PoleSkrotu.start()` wywołuje `void this.most.skrotWstrzymaj(true)` (`skrot.ts:128`), co wyrejestrowuje globalny skrót w systemie. Przywrócenie skrótu przy zamykaniu okna polega wyłącznie na asynchronicznym `invoke("skrot_wstrzymaj", { wstrzymaj: false })` wywołanym z `blur` / `pagehide` (`skrot.ts:104-105`).
  W WebView2 podczas niszczenia okna (`getCurrentWindow().close()` lub kliknięcie systemowego `X`) wywołania IPC `invoke` wysłane z `pagehide` mogą nie dotrzeć do backendu przed zniszczeniem kontekstu WebView, a w `src-tauri/src/lib.rs:123-137` brak obsługi `WindowEvent::Destroyed` dla `OKNO_USTAWIEN`. Jeśli wiadomość IPC przepadnie, `skrot_wstrzymany` zostanie w backendzie na `true` i globalny skrót przestanie działać aż do ponownego otwarcia ustawień.
- **Proponowana poprawka:**
  W `src-tauri/src/lib.rs` w `.on_window_event(...)` dodać gałąź:
  ```rust
  WindowEvent::Destroyed if okno.label() == aplikacja::OKNO_USTAWIEN => {
      if let Some(apl) = apl(okno.app_handle()) {
          apl.skrot_wstrzymaj(false);
      }
  }
  ```

---

#### Z-21. Brak lokalnej obsługi błędów (`catch`) w kilku wywołaniach `most.*` we frontendzie

- **Waga:** niska
- **Status:** POTWIERDZONE
- **Plik i linia:** `src/main.ts:100`, `src/most.ts:87-89`, `src/kreator.ts:185-191`, `src/kreator.ts:216-222`, `src/ustawienia.ts:65-69`, `src/ustawienia.ts:183-186`
- **Dowód:**
  Większość wywołań `most.*` ma jawne `.catch(blad)` lub `try / catch`, ale w kilku miejscach obietnice są porzucane przez `void` bez `.catch(...)`:
  - `src/main.ts:100`: `void most.dopasuj(szer, wys);`
  - `src/most.ts:88`: `void invoke("przeciagnij");` (a komenda `przeciagnij` w `aplikacja.rs:413-416` zwraca `Result<(), String>` i może odrzucić obietnicę błędem `start_dragging()`)
  - `src/kreator.ts:185-191` i `216-222`: `wybierzPlik` / `wybierzFolder` wewnątrz `void akcja()` bez `try / catch`
  - `src/ustawienia.ts:65-69`, `183-186`: `void wybor(w.value).then(...)` oraz `void most.zamknijOkno()`, przy czym w `ustawienia.html` (`src/ustawienia.ts`) **nie zarejestrowano** globalnych nasłuchiwaczy `window.addEventListener("error", ...)` i `unhandledrejection` (są one tylko w `src/main.ts:62-63`), więc nieobsłużone odrzucenia w oknie ustawień nie trafią nawet do pliku logu!
- **Proponowana poprawka:**
  Dodać w `src/ustawienia.ts` globalne nasłuchiwacze `error` i `unhandledrejection` przekazujące błędy do `most.zglosBlad(...)` (tak jak w `src/main.ts:62-63`), a w `przyciskWyboru` (`kreator.ts`) i `sciezka` (`ustawienia.ts`) łapać błędy dialogów i pokazywać komunikat lub zapisywać w logu.

---

### 5. Luki w testach

---

#### Z-22. Luki w pokryciu testami kluczowych zachowań ze `SPEC.md`

- **Waga:** średnia
- **Status:** POTWIERDZONE
- **Plik i linia:** `src-tauri/src/dziennik.rs:319-323`, `src-tauri/src/plik_zadan.rs:562-575`, `src-tauri/src/rdzen.rs:2193-2218`, `vitest.config.ts:1-7`, `src/*.test.ts`
- **Dowód:**
  Analiza istniejących testów względem `SPEC.md` wykazuje następujące luki (trzy pierwsze bezpośrednio przepuściły błędy Z-01, Z-07 i Z-08):
  1. **Zapis dziennika na dysk z BOM (`SPEC.md:389`):** test `zachowuje_crlf_i_bom` (`dziennik.rs:319`) wywołuje `aktualizuj_tresc` na napisie w pamięci i nigdy nie testuje `dziennik::zapisz` na prawdziwym pliku z UTF-8 BOM.
  2. **`Cofnij` przy zduplikowanych zadaniach w jednym projekcie (`SPEC.md:492`):** `plik_zadan.rs` ma test `duplikaty_najpierw_otwarte` dla `dopisz_czas`, ale nie ma odpowiadającego mu testu dla `odznacz_zrobione` po ukończeniu drugiego zadania o tej samej nazwie.
  3. **Natychmiastowy `tick` po `Zrobione` dla sesji nocnej przechodzącej przez północ (`SPEC.md:492, 495`):** test `sesja_przez_polnoc_nalezy_do_dnia_startu` (`rdzen.rs:2193-2218`) wywołuje `r.tick` dopiero po upływie pełnych 10 sekund (`00:15:10`), zamiast tak jak prawdziwy wątek `monitor` — ułamek sekundy po `r.zrobione` (`00:15:00`), przez co nie wykrywa natychmiastowego ucięcia `Gratulacje`.
  4. **Bezczynność („Jesteś?”) przy pełnym ekranie na drugim monitorze (`SPEC.md:490`):** brak testu w `rdzen.rs` i `system/mod.rs` sprawdzającego, że pełny ekran na monitorze innym niż monitor widgetu blokuje stan `Jesteś?`, ale nie chowa widgetu.
  5. **Brak testów komponentów UI i klas ze stanem we frontendzie (`SPEC.md` B4, B6, B7):** Vitest działa wyłącznie w środowisku Node (`vitest.config.ts`) i testuje czyste funkcje (`format.ts`, `przerwa.ts`, `kolory.css`, `kwestia()`, pomocniki `kreator.ts`, `skrot.ts`, `dzwiek.ts`, `dodawanie.ts`). Żadnego testu nie mają:
     - `src/postac.ts` (wybór póz dla wszystkich 9 stanów, priorytet `blisko` nad wariantami `konsola`/`kubek`, zatrzymywanie timerów przy `widocznosc(false)`, zanikanie 150 ms),
     - `src/widok.ts` (renderowanie paneli, stan przycisku „Zaczynamy”, przekroczenie szacunku >100% i celu >120%, odliczanie „Przerwa (odejmij X min)”),
     - klasa `Dymek` w `src/dymki.ts` (znikanie po 6 s / 10 s, działanie `usunPrzycisk()`, kliknięcie w przycisk `Cofnij` vs kliknięcie w tło dymka),
     - przebieg 5 kroków klasy `Kreator`, walidacja formularza `Dodawanie`, nagrywanie w `PoleSkrotu` i odsłuch na żywo w `PoleGlosnosci`.
  6. **Brak testów warstwy `aplikacja.rs`:** brak testów współbieżności zapisu `ustawienia.json` oraz logiki debounce `plik_zmieniony`.
- **Proponowana poprawka:**
  Dopisać brakujące testy w Ruście dla punktów 1–4, wydzielić czysty selektor póz z `Postac` (bez DOM, tak jak zrobiono w `przerwa.ts`) albo dodać lekkie środowisko DOM (np. `happy-dom` / `jsdom`, z uzasadnieniem w PR zgodnie z `AGENTS.md`) dla testów `Postac`, `Dymek` i `widok.ts`.

---

### 6. Zależności i CI

---

#### Z-23. Luki w workflow CI: brak `--all-targets` dla Clippy na Windows, brak `npm test` na Windows, brak `--locked` oraz brak zawężenia `permissions`

- **Waga:** średnia
- **Status:** POTWIERDZONE
- **Plik i linia:** `.github/workflows/ci.yml:1-70`
- **Dowód:**
  1. W `.github/workflows/ci.yml:56-58` krok Clippy na Windows nie używa flagi `--all-targets` (w przeciwieństwie do Linuksa w linii 34 i instrukcji z `AGENTS.md`):
     ```yaml
     - name: Clippy (kod Windows)
       run: cargo clippy -- -D warnings
       working-directory: src-tauri
     ```
     Przez to kod `#[cfg(test)]` kompilowany pod Windows nie jest sprawdzany przez Clippy.
  2. Job `windows` (`ci.yml:40-70`) w ogóle nie uruchamia `npm test`, więc testy TypeScript operujące na ścieżkach systemu plików (`dymki.test.ts`, `kolory.test.ts`, `kreator.test.ts`) działają wyłącznie na Linuksie.
  3. Wywołania `cargo clippy`, `cargo test` i `npm run tauri build` nie przekazują flagi `--locked` do Cargo, więc CI nie weryfikuje ścisłej aktualności `src-tauri/Cargo.lock`.
  4. W pliku workflow brak bloku `permissions: contents: read` (zasada najmniejszych uprawnień dla `GITHUB_TOKEN`) oraz bloku `concurrency` anulującego przestarzałe przebiegi na tym samym PR.
- **Proponowana poprawka:**
  *(Uwaga: zgodnie z `AGENTS.md` zmian w `.github/workflows/` należy dokonywać wyłącznie na wyraźną prośbę właściciela).* Po uzyskaniu zgody: dodać `--all-targets --locked` do `cargo clippy` na obu systemach, `--locked` do `cargo test`, `npm test` w jobie `windows` oraz ustawić na poziomie workflow `permissions: { contents: read }` i `concurrency`.

---

### 7. Czytelność: największe pliki i powtarzający się kod

---

#### Z-24. Rozbudowane pliki monolityczne oraz zduplikowany i martwy kod w CSS i TypeScript

- **Waga:** niska
- **Status:** POTWIERDZONE
- **Plik i linia:**
  - Największe pliki w repozytorium:
    - `src-tauri/src/rdzen.rs` (`3282` linie: ~1686 linii kodu + ~1596 linii testów)
    - `src-tauri/src/dzwiek.rs` (`1510` linii: ~720 linii kodu + ~790 linii testów)
    - `src-tauri/src/aplikacja.rs` (`1123` linie)
    - `src/style.css` (`1079` linii)
    - `src-tauri/src/plik_zadan.rs` (`677` linii)
    - `src-tauri/src/okno.rs` (`609` linii)
    - `src-tauri/src/ustawienia.rs` (`585` linii)
  - Powtórzenia i martwy kod:
    - `src/style.css:691-701` oraz `src/style.css:963-971`
    - `src/style.css:621-664` oraz `src/ustawienia.css:41-84`
    - `src/style.css:523-525` i `src/style.css:683-689`
    - `src/kreator.ts:205-230, 272-288` oraz `src/ustawienia.ts:41-71`
    - `src/main.ts:61`, `src/ustawienia.ts:17`, `src/kreator.ts:355`, `src/dodawanie.ts:91`
    - `src/format.ts:34-36` oraz `src/zastepcza.ts:261-264`
- **Dowód:**
  1. **Monolity w Ruście:**
     - `rdzen.rs` (3282 linie) skupia w jednym pliku wszystkie struktury DTO (`Migawka` i ~12 podstruktur), maszynę stanów, kolejkę ponowień I/O, logikę reakcji końca dnia oraz 1600 linii testów.
     - `aplikacja.rs` (1123 linie) łączy w jednej strukturze `Aplikacja` (21 pól, z czego 11 to `Mutex`) zarządcę położenia okna/monitorów, skrót globalny, menu zasobnika i kontekstowe, obserwator `notify`, obsługę ustawień/kreatora oraz pętlę wątku `monitor`.
  2. **Duplikacje i martwe reguły w CSS:**
     - Klasa `.link` jest zdefiniowana w `src/style.css` **dwukrotnie** (linie `691-701` i `963-971`).
     - Style `.przyciski`, `.przycisk`, `:hover`, `:focus-visible` i `.przycisk.glowny` są skopiowane między `src/style.css:621-664` a `src/ustawienia.css:41-84`, mimo że oba okna ładują wspólny plik `src/formularz.css`.
     - Reguły `.zapasowe .zadanie` (`src/style.css:523-525`) i `.zrobione-lista` (`src/style.css:683-689`) to pozostałości po wersji 1 — żaden element w kodzie ich nie używa.
  3. **Duplikacje i nieużywane eksporty w TypeScript:**
     - Funkcje budujące pole ścieżki z przyciskiem dialogu oraz przełącznik z opisem są napisane osobno w `src/kreator.ts` i `src/ustawienia.ts`; podobnie konwersja błędu `unknown → string` jest powielona w 4 plikach.
     - Funkcja `godzinaBezZera` (`src/format.ts:34-36`, z komentarzem o „godzinie planu” z wersji 1 w teście) oraz `svgIkony` (`src/zastepcza.ts:261-264`) nie są nigdzie używane w kodzie produkcyjnym.
- **Proponowana poprawka:**
  1. W `rdzen.rs` i `dzwiek.rs` wydzielić moduły testów do osobnych plików `rdzen/testy.rs` i `dzwiek/testy.rs` (lub przekształcić `rdzen` w katalog z podmodułami `widok.rs` i `testy.rs`), co od razu skróci oba pliki o połowę bez zmiany publicznego API.
  2. Przenieść wspólne style `.przycisk*` do `src/formularz.css`, usunąć duplikat `.link` i martwe klasy z wersji 1 w `src/style.css` oraz nieużywane funkcje `godzinaBezZera` i `svgIkony`.
  3. Wydzielić wspólne kontrolki formularza (`poleSciezki`, `przelacznik`, `opisBledu`) do małego modułu `src/pola.ts`.

---

## 5 najważniejszych rzeczy do zrobienia (od najważniejszej)

1. **Naprawić przejście przez północ po kliknięciu `Zrobione` (Z-01):** odroczyć `Rdzen::zmiana_dnia` do zakończenia 10-sekundowej fazy `Gratulacje` (okna na `Cofnij`) i dodać test z natychmiastowym `tick` po północy.
2. **Naprawić odznaczanie duplikatów zadań przy kliknięciu `Cofnij` (Z-07):** w `plik_zadan::odznacz_zrobione` odznaczać właśnie ukończone zadanie (preferując dzisiejszą datę `✅ RRRR-MM-DD` / szukając od końca projektu), zamiast pierwszego historycznego zadania od góry pliku, wraz z testem regresyjnym.
3. **Uszczelnić zapis plików i synchronizację między wątkami w Ruście (Z-08, Z-09, Z-10, Z-11):** zachować UTF-8 BOM w `dziennik::zapisz` (z testem na pliku dyskowym), zserializować zapis `ustawienia.json` i unikalność nazw `.tmp` w `zapisz_atomowo`, dodać numer wersji migawki w `Aplikacja::wykonaj` oraz atomowe czyszczenie `plik_zmieniony`.
4. **Doprowadzić zachowanie na wielu monitorach i w stanie `Praca` do pełnej zgodności ze `SPEC.md` (Z-02, Z-03, Z-04):** blokować pytanie `Jesteś?` przy pełnym ekranie na *dowolnym* monitorze, ograniczyć pozę `sit_watch_front` wyłącznie do wariantu `patrzy` oraz przekazywać wysokość obszaru roboczego docelowego monitora z backendu do wyliczania `--panel-max-h`.
5. **Uszczelnić walidację ścieżek i sprzątanie timerów/zasobów w oknach (Z-15, Z-18, Z-20):** wymusić rozszerzenie `.md`/`.markdown` dla `plik_zadan` przed zapisem i `open_path`, zatrzymywać wszystkie timery animacji w `Postac.widocznosc(false)` oraz resetować `skrot_wstrzymaj(false)` na `WindowEvent::Destroyed` okna ustawień.
