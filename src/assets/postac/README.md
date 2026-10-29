# Grafiki postaci

Gotowe grafiki z części A specyfikacji (`SPEC.md`, A3 i A5): 18 póz, `icon.png` i
opis `postac.json`. Bez nich widget rysuje postać zastępczą (`src/zastepcza.ts`).

Umowa dla plików:

- nazwy dokładnie jak w A3, małymi literami, np. `sit_watch.png`;
- przezroczyste tło (zieleń z Gemini usunięta), PNG RGBA;
- wszystkie pozy na płótnie tej samej wielkości, w tej samej skali i pozycji, więc
  podmiana klatek nie przesuwa postaci;
- `postac.json` opisuje płótno (`plotno_px`), rozmiar postaci na ekranie
  (`wyswietlanie_css_px`) oraz linię stóp (`stopy_y_px`) albo siedziska
  (`siedzisko_y_px`) każdej pozy;
- `icon.png` (1024 × 1024) nie jest wyświetlany w widgecie. Ikony aplikacji robi
  się z niego poleceniem `npx tauri icon src/assets/postac/icon.png -o src-tauri/icons`.

Widget sam wykrywa pliki w tym folderze i bierze z `postac.json` rozmiar postaci
oraz dwie linie: stóp z `stand_idle` i siedziska z `sit_watch`. W Pracy i Pauzie
linia siedziska wypada na górnej krawędzi panelu, a reszta postaci chowa się za
nim. W pozach stojących na panelu postać stoi na jego krawędzi. Postać wyższa niż
260 px oddycha delikatniej (skala do 1,008, jak w wariancie B z A2b).

Brakującą grafikę opcjonalną zastępuje podstawowa poza z tej samej grupy (np.
`wave_2` przez `wave_1`), a brakującą obowiązkową postać zastępcza.
