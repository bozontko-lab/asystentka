// Piaskownica do testów (krok 2 z B9): widget na plikach z przyklady/, z
// dzisiejszą datą, w osobnym katalogu .piaskownica/. Prawdziwe
// %APPDATA%\Asystentka, Dysk i vault zostają nietknięte.
//
//   npm run piaskownica                 przygotuj pliki i uruchom `tauri dev`
//   npm run piaskownica -- --od-nowa    wyczyść piaskownicę przed startem
//   npm run piaskownica -- --v1         ustawienia z wersji 1: kreator wypełnia się nimi
//   npm run piaskownica -- --kreator    bez pliku ustawień: kreator od zera
//   npm run piaskownica -- --bez-planu  bez dzisiejszego planu (stan „Czeka na plan”)
//   npm run piaskownica -- --dodaj-plan dopisz dzisiejszy plan i zakończ
//                                       (jak poranne zadanie; widget zauważy go w 30 s)
//   npm run piaskownica -- --tylko-pliki przygotuj pliki bez uruchamiania

import { spawn } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const argumenty = new Set(process.argv.slice(2));
const repo = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const katalog = join(repo, ".piaskownica");

if (argumenty.has("--od-nowa")) rmSync(katalog, { recursive: true, force: true });

const dysk = join(katalog, "Dysk", "Asystentka");
const plany = join(dysk, "plany");
const plikStanu = join(dysk, "stan-zadan.json");
const dziennik = join(katalog, "vault", "Dziennik pracy");
const dane = join(katalog, "AppData", "Asystentka");
for (const folder of [plany, dziennik, dane]) mkdirSync(folder, { recursive: true });

const dwie = (n) => String(n).padStart(2, "0");
const teraz = new Date();
const dzis = `${teraz.getFullYear()}-${dwie(teraz.getMonth() + 1)}-${dwie(teraz.getDate())}`;

function dodajPlan() {
  const plan = JSON.parse(readFileSync(join(repo, "przyklady", "plan-2026-10-30.json"), "utf8"));
  plan.data = dzis;
  plan.utworzono = new Date().toISOString();
  const sciezka = join(plany, `plan-${dzis}.json`);
  writeFileSync(sciezka, `${JSON.stringify(plan, null, 2)}\n`);
  console.log(`plan na dziś: ${sciezka}`);
}

if (argumenty.has("--dodaj-plan")) {
  dodajPlan();
  process.exit(0);
}

if (!argumenty.has("--bez-planu")) dodajPlan();
if (!existsSync(plikStanu)) copyFileSync(join(repo, "przyklady", "stan-zadan.json"), plikStanu);

const vault = join(katalog, "vault");
const ustawienia = join(dane, "ustawienia.json");
const plikZadan = join(vault, "Zadania.md");
if (argumenty.has("--kreator")) rmSync(ustawienia, { force: true });
else if (!existsSync(ustawienia)) {
  const wspolne = {
    imie: "Dżiordżio",
    godzina_konca_dnia: "22:00",
    prog_bezczynnosci_min: 15,
    przypomnienie_przerwy_min: 50,
    ukryj_przed_nagrywaniem: true,
    chowaj_przy_pelnym_ekranie: true,
    skrot_pokaz_ukryj: "Ctrl+Alt+Space",
  };
  // Pola z wersji 1 zostają, dopóki widget korzysta jeszcze z planu na Dysku.
  const v1 = { folder_planow: plany, plik_stanu: plikStanu, folder_dziennika: dziennik, godzina_planu: "07:40" };
  // --v1: plik ustawień z wersji 1, bez plik_zadan, więc otwiera się kreator wypełniony tymi danymi.
  const wartosci = argumenty.has("--v1")
    ? { ...wspolne, ...v1 }
    : { ...wspolne, plik_zadan: plikZadan, folder_dziennika: dziennik, cel_dzienny_min: 240, autostart: false, ...v1 };
  writeFileSync(ustawienia, `${JSON.stringify(wartosci, null, 2)}\n`);
}
if (!argumenty.has("--v1") && !argumenty.has("--kreator") && !existsSync(plikZadan)) {
  copyFileSync(join(repo, "przyklady", "Zadania.md"), plikZadan);
}

console.log(`piaskownica: ${katalog}`);
console.log(`  ustawienia: ${ustawienia}`);
console.log(`  dziennik:   ${dziennik}`);
if (argumenty.has("--tylko-pliki")) process.exit(0);

const tauri = spawn("npx", ["tauri", "dev"], {
  cwd: repo,
  stdio: "inherit",
  shell: process.platform === "win32",
  env: { ...process.env, ASYSTENTKA_KATALOG: dane },
});
tauri.on("exit", (kod) => process.exit(kod ?? 0));
