// Piaskownica do testów: widget na plikach z przyklady/ w osobnym katalogu
// .piaskownica/. Prawdziwe %APPDATA%\Asystentka i vault zostają nietknięte.
//
//   npm run piaskownica                 przygotuj pliki i uruchom `tauri dev`
//   npm run piaskownica -- --od-nowa    wyczyść piaskownicę przed startem
//   npm run piaskownica -- --v1         ustawienia z wersji 1: kreator wypełnia się nimi
//   npm run piaskownica -- --kreator    bez pliku ustawień: kreator od zera
//   npm run piaskownica -- --tylko-pliki przygotuj pliki bez uruchamiania
//
// Pliki w .piaskownica/:
//   vault/Zadania.md, vault/Dziennik pracy/   plik zadań i dziennik (otwórz vault w Obsidianie)
//   AppData/Asystentka/                       ustawienia.json, dzien.json, okno.json

import { spawn } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const argumenty = new Set(process.argv.slice(2));
const repo = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const katalog = join(repo, ".piaskownica");

if (argumenty.has("--od-nowa")) rmSync(katalog, { recursive: true, force: true });

const vault = join(katalog, "vault");
const dziennik = join(vault, "Dziennik pracy");
const dane = join(katalog, "AppData", "Asystentka");
for (const folder of [dziennik, dane]) mkdirSync(folder, { recursive: true });

const ustawienia = join(dane, "ustawienia.json");
const plikZadan = join(vault, "Zadania.md");
const wspolne = {
  imie: "Dżiordżio",
  godzina_konca_dnia: "22:00",
  prog_bezczynnosci_min: 15,
  przypomnienie_przerwy_min: 50,
  ukryj_przed_nagrywaniem: true,
  chowaj_przy_pelnym_ekranie: true,
};

if (argumenty.has("--kreator")) {
  rmSync(ustawienia, { force: true });
} else if (!existsSync(ustawienia)) {
  const wartosci = argumenty.has("--v1")
    ? // Plik ustawień z wersji 1: bez plik_zadan, więc otwiera się kreator wypełniony tymi danymi.
      {
        ...wspolne,
        // Stary domyślny skrót: widget zamienia go na nowy przy pierwszym starcie wersji 2.1.
        skrot_pokaz_ukryj: "Ctrl+Alt+Space",
        folder_planow: join(katalog, "Dysk", "Asystentka", "plany"),
        plik_stanu: join(katalog, "Dysk", "Asystentka", "stan-zadan.json"),
        folder_dziennika: dziennik,
        godzina_planu: "07:40",
      }
    : {
        ...wspolne,
        skrot_pokaz_ukryj: "Ctrl+Alt+Shift+Space",
        plik_zadan: plikZadan,
        folder_dziennika: dziennik,
        cel_dzienny_min: 240,
        autostart: false,
      };
  writeFileSync(ustawienia, `${JSON.stringify(wartosci, null, 2)}\n`);
}

// Przy --v1 i --kreator plik zadań powstaje dopiero w kreatorze.
if (!argumenty.has("--v1") && !argumenty.has("--kreator") && !existsSync(plikZadan)) {
  copyFileSync(join(repo, "przyklady", "Zadania.md"), plikZadan);
}

console.log(`piaskownica: ${katalog}`);
console.log(`  ustawienia: ${ustawienia}`);
console.log(`  zadania:    ${plikZadan}`);
console.log(`  dziennik:   ${dziennik}`);
if (argumenty.has("--tylko-pliki")) process.exit(0);

const tauri = spawn("npx", ["tauri", "dev"], {
  cwd: repo,
  stdio: "inherit",
  shell: process.platform === "win32",
  env: { ...process.env, ASYSTENTKA_KATALOG: dane },
});
tauri.on("exit", (kod) => process.exit(kod ?? 0));
