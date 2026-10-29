// Piaskownica do testów (krok 2 z B9): widget na plikach z przyklady/, z
// dzisiejszą datą, w osobnym katalogu .piaskownica/. Prawdziwe
// %APPDATA%\Asystentka, Dysk i vault zostają nietknięte.
//
//   npm run piaskownica                 przygotuj pliki i uruchom `tauri dev`
//   npm run piaskownica -- --od-nowa    wyczyść piaskownicę przed startem
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

const ustawienia = join(dane, "ustawienia.json");
if (!existsSync(ustawienia)) {
  const wartosci = {
    imie: "Dżiordżio",
    folder_planow: plany,
    plik_stanu: plikStanu,
    folder_dziennika: dziennik,
    godzina_planu: "07:40",
    godzina_konca_dnia: "22:00",
    prog_bezczynnosci_min: 15,
    przypomnienie_przerwy_min: 50,
    ukryj_przed_nagrywaniem: true,
    chowaj_przy_pelnym_ekranie: true,
    skrot_pokaz_ukryj: "Ctrl+Alt+Space",
  };
  writeFileSync(ustawienia, `${JSON.stringify(wartosci, null, 2)}\n`);
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
