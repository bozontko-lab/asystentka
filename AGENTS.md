# AGENTS.md: zasady dla agentów AI (Gemini/Antigravity, Claude Code i inne)

Ten plik czytają wszyscy agenci pracujący nad repozytorium. Źródłem prawdy o zachowaniu programu jest `SPEC.md` (wersja 2.1); stan wdrożenia i decyzje są w `README.md`.

## Czym jest projekt

Widget na Windows 10/11: Avatar AI w rogu ekranu, wybór zadań z `Zadania.md`, licznik czasu z pauzą, zapis sesji do pliku zadań i dziennika w Obsidianie, podsumowanie dnia. Działa wyłącznie na plikach na dysku: **bez internetu i bez LLM**. Tej zasady nie wolno łamać (żadnych wywołań sieciowych, telemetrii, zewnętrznych API).

Stack: Tauri 2 (Rust, `src-tauri/`) + frontend w TypeScript, HTML i CSS bez frameworka (`src/`). Vite, Vitest.

## Polecenia

- Instalacja: `npm install`
- Testy frontendu: `npm test`
- Testy Rust: `cargo test --manifest-path src-tauri/Cargo.toml`
- Lint Rust: `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
- Format Rust: `cargo fmt --manifest-path src-tauri/Cargo.toml`
- Uruchomienie na przykładach (bezpieczne): `npm run piaskownica`
- Podgląd stanów w przeglądarce: `npm run dev`, potem `http://localhost:1420/?stan=praca`

Zmiana jest gotowa dopiero, gdy oba zestawy testów przechodzą. Nie wyłączaj ani nie osłabiaj istniejących testów, żeby przeszły.

## Zasady architektury (nie zmieniać bez zgody właściciela)

- Pliki obsługuje Rust, nie wtyczka file-system: zapis atomowy (plik tymczasowy + zmiana nazwy), świeży odczyt tuż przed zapisem, zachowanie BOM i CRLF, zmiana dokładnie jednej linii, kolejka ponowień.
- Widget nigdy nie nadpisuje istniejącego `Zadania.md` ani notatek w dzienniku poza własnym blokiem.
- `src-tauri/src/rdzen.rs` (stany i przejścia) nie zależy od Tauri.
- Kolory tylko jako zmienne w `src/kolory.css`. Żadnych kolorów wpisanych na sztywno (pilnuje tego test).
- Każdy dymek z backendu musi mieć wariant w `src/kwestie.json` (pilnuje tego test).
- Okno nie kradnie fokusu (`focusable: false`).
- Kod Windows (`src/system/win.rs`) musi się kompilować w CI także pod Linuksem w gałęziach `cfg`.

## Konwencje

- Nazwy w kodzie, komentarze, komunikaty i dokumentacja: po polsku, jak w istniejącym kodzie.
- Małe, skupione zmiany. Jedna sprawa = jedna gałąź = jeden pull request.
- Nowa funkcja albo poprawka błędu: najpierw test, który pokazuje problem, potem poprawka.
- Po zmianie zachowania zaktualizuj `README.md` (tabela stanu wdrożenia, sekcja decyzji). Rozbieżność z `SPEC.md` zgłoś, zamiast po cichu ją wprowadzać.

## Czego nie robić

- Nie commituj do `main`; pracuj na gałęzi.
- Nie dodawaj zależności (crate ani npm) bez uzasadnienia w opisie PR.
- Nie zmieniaj `.github/workflows/` bez wyraźnej prośby.
- Nie uruchamiaj `npm run tauri dev` na prawdziwych ustawieniach (`%APPDATA%\Asystentka\`) ani prawdziwym vaulcie; do testów używaj `npm run piaskownica`.
- Nie czytaj ani nie commituj `.piaskownica/`, logów i prywatnych plików użytkownika.

## Rzeczy sprawdzalne tylko na prawdziwym Windows

Zdarzenia systemowe (blokada, uśpienie, bezczynność, pełny ekran, ukrycie przed nagrywaniem, OBS), wiele monitorów i skalowanie 150%, dźwięk w słuchawkach. Przy takich zmianach dopisz w PR, co właściciel ma sprawdzić ręcznie (lista „Sprawdzenie na Windows” w README).
