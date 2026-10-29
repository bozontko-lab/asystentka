# Grafiki postaci

Tu trafiają gotowe PNG z części A specyfikacji (`SPEC.md`, A3 i A5). Dopóki
ich nie ma, widget rysuje postać zastępczą (`src/zastepcza.ts`).

Umowa dla plików:

- nazwy dokładnie jak w A3, małymi literami, np. `sit_watch.png`;
- przezroczyste tło (zieleń z Gemini usunięta), PNG RGBA;
- wszystkie pozy na kwadratowym płótnie tej samej wielkości i w tej samej skali;
- dół postaci (stopy albo miejsce siedzenia) na dolnej krawędzi płótna,
  postać wyśrodkowana w poziomie;
- `ref_master.png` i `icon.png` nie są wyświetlane: pierwszy to wzorzec do
  promptów, z drugiego robi się ikony aplikacji poleceniem
  `npx tauri icon src/assets/postac/icon.png`.

Widget sam wykrywa, które pliki są w tym folderze. Brakująca grafika
opcjonalna jest zastępowana podstawową pozą z tej samej grupy (np. `wave_2`
przez `wave_1`), a brakująca obowiązkowa postacią zastępczą.
