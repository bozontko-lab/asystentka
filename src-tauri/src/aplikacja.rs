//! Spina rdzeń z Tauri: zdarzenia dla frontendu, okno w rogu ekranu,
//! zasobnik, skrót globalny, zdarzenia Windows i wątek pilnujący terminów.
//!
//! Zasada: nie wołamy API okna ani menu, trzymając blokadę rdzenia, bo te
//! wywołania mogą czekać na wątek główny, który czeka na tę blokadę.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use tauri::menu::MenuEvent;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition};
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use tauri_plugin_opener::OpenerExt;

use crate::czas::{self, Chwila};
use crate::magazyn::Magazyn;
use crate::obsidian;
use crate::okno::{self, Ekran, Obszar, Prostokat};
use crate::rdzen::{Migawka, Rdzen, Stan, Zdarzenie};
use crate::sesja::PowodPauzy;
use crate::skrot;
use crate::system::{self, ZdarzenieSystemowe};
use crate::ustawienia::{self, PodpowiedzKreatora, Ustawienia};
use crate::zasobnik::{self, StanMenu, Zasobnik};

pub const OKNO: &str = "main";
pub const OKNO_USTAWIEN: &str = "ustawienia";

const CO_ILE_CYKL: Duration = Duration::from_secs(30);
const CO_ILE_PELNY_EKRAN: Duration = Duration::from_secs(2);
const CO_ILE_KURSOR: Duration = Duration::from_millis(500);
const PO_PRZECIAGNIECIU: Duration = Duration::from_millis(400);
/// Tyle po kliknięciu w postać przesunięcie okna liczy się jako ruch użytkownika.
const RECZNE_PRZESUNIECIE: Duration = Duration::from_secs(30);
const ODSTEP_PO_ZAPISIE: Duration = Duration::from_millis(300);
/// Glitch wyjścia trwa 400 ms; okno chowamy, gdy się skończy.
const GLITCH_WYJSCIA: Duration = Duration::from_millis(420);
/// Odległość kursora, przy której postać patrzy na Ciebie (piksele logiczne).
const BLISKO_PX: f64 = 300.0;
/// Po starcie dymek o zajętym skrócie czeka, aż skończy się powitanie i następny dymek.
const SKROT_ZAJETY_PO_STARCIE_S: i64 = 13;

#[derive(Default)]
struct StanOkna {
    rozmiar: Option<(f64, f64)>,
    prostokat: Option<Prostokat>,
    /// Monitor, na którym stoi widget (po ostatnim ustawieniu okna).
    ekran: Option<Ekran>,
    /// Monitory z ostatniego sprawdzenia: zmiana zestawu przestawia okno.
    monitory: Option<Vec<Ekran>>,
    widoczna: Option<bool>,
    pokolenie: u64,
    /// Ostatni ruch okna, który zrobił użytkownik; przeciąganie kończy się po chwili spokoju.
    przesuniete: Option<Instant>,
    /// Ostatnie kliknięcie w postać (początek przeciągania).
    reka_od: Option<Instant>,
    /// Ostatni ruch okna zrobiony przez system, do poprawienia po chwili spokoju.
    do_poprawy: Option<Instant>,
}

pub struct Aplikacja {
    app: AppHandle,
    rdzen: Mutex<Rdzen>,
    budzik: Mutex<Sender<()>>,
    gotowy: AtomicBool,
    okno: Mutex<StanOkna>,
    hwnd: AtomicIsize,
    zasobnik: OnceLock<Zasobnik>,
    ostatnia_migawka: Mutex<String>,
    /// Skrót zarejestrowany w systemie w tej chwili.
    skrot: Mutex<Option<String>>,
    /// Skrót z ustawień, który widget chce mieć zarejestrowany.
    skrot_docelowy: Mutex<String>,
    /// Pole skrótu w ustawieniach nagrywa kombinację: nasz skrót nie może jej przechwycić.
    skrot_wstrzymany: AtomicBool,
    ustawienia_zmienione: Mutex<Option<SystemTime>>,
    pelny_ekran: AtomicBool,
    kursor_blisko: AtomicBool,
    ochrona: Mutex<Option<bool>>,
    obserwator: Mutex<Option<(PathBuf, notify::RecommendedWatcher)>>,
    plik_zmieniony: Mutex<Option<Instant>>,
}

fn zablokuj<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn stan_menu(r: &Rdzen) -> StanMenu {
    StanMenu::z(&r.mozliwosci(), r.ust.ukryj_przed_nagrywaniem, r.ust.postac())
}

/// `%APPDATA%\Asystentka`, albo katalog z `ASYSTENTKA_KATALOG` (piaskownica do testów).
fn katalog_danych(app: &AppHandle) -> Result<PathBuf, Box<dyn Error>> {
    if let Some(katalog) = std::env::var_os("ASYSTENTKA_KATALOG") {
        return Ok(PathBuf::from(katalog));
    }
    Ok(app.path().config_dir()?.join("Asystentka"))
}

pub fn uruchom(app: &mut tauri::App) -> Result<(), Box<dyn Error>> {
    let handle = app.handle().clone();
    let magazyn = Magazyn::new(katalog_danych(&handle)?);
    let sciezka_ustawien = magazyn.ustawienia();
    log::info!("dane widgetu: {}", magazyn.katalog.display());
    let wynik = ustawienia::wczytaj_przy_starcie(&sciezka_ustawien);
    let (kreator, blad_ustawien) = match &wynik {
        ustawienia::Wynik::Kreator(_) => {
            log::info!("brak pola plik_zadan w {}: otwieram kreator", sciezka_ustawien.display());
            (true, false)
        }
        ustawienia::Wynik::Wczytane(_) => (false, false),
        ustawienia::Wynik::Blad(_, e) => {
            log::warn!("{e}; używam wartości domyślnych");
            (false, true)
        }
    };
    let ust = wynik.ustawienia().clone();

    let teraz = czas::teraz();
    let mut rdzen = Rdzen::nowy(magazyn, ust.clone(), teraz);
    if kreator {
        rdzen.wymus_kreator();
    }
    rdzen.start(teraz);
    if blad_ustawien {
        rdzen.blad_ustawien();
    }
    let menu = stan_menu(&rdzen);

    let (budzik, sygnaly) = mpsc::channel();
    let apl = Arc::new(Aplikacja {
        app: handle.clone(),
        rdzen: Mutex::new(rdzen),
        budzik: Mutex::new(budzik),
        gotowy: AtomicBool::new(false),
        okno: Mutex::new(StanOkna::default()),
        hwnd: AtomicIsize::new(0),
        zasobnik: OnceLock::new(),
        ostatnia_migawka: Mutex::new(String::new()),
        skrot: Mutex::new(None),
        skrot_docelowy: Mutex::new(String::new()),
        skrot_wstrzymany: AtomicBool::new(false),
        ustawienia_zmienione: Mutex::new(fs::metadata(&sciezka_ustawien).and_then(|m| m.modified()).ok()),
        pelny_ekran: AtomicBool::new(false),
        kursor_blisko: AtomicBool::new(false),
        ochrona: Mutex::new(None),
        obserwator: Mutex::new(None),
        plik_zmieniony: Mutex::new(None),
    });
    app.manage(Arc::clone(&apl));

    if let Some(okno) = handle.get_webview_window(OKNO) {
        apl.hwnd.store(system::uchwyt(&okno).unwrap_or(0), Ordering::SeqCst);
    }
    apl.zastosuj_ochrone(ust.ukryj_przed_nagrywaniem);

    let zasobnik = Zasobnik::utworz(
        &handle,
        &menu,
        |app, zdarzenie| {
            if let Some(apl) = app.try_state::<Arc<Aplikacja>>() {
                apl.inner().menu(zdarzenie);
            }
        },
        |app| {
            if let Some(apl) = app.try_state::<Arc<Aplikacja>>() {
                apl.inner().wykonaj(|r, _| r.przelacz_widocznosc());
            }
        },
    )?;
    let _ = apl.zasobnik.set(zasobnik);

    if !apl.zarejestruj_skrot(&ust.skrot_pokaz_ukryj) {
        let po = teraz + chrono::Duration::seconds(SKROT_ZAJETY_PO_STARCIE_S);
        zablokuj(&apl.rdzen).skrot_zajety(&ust.skrot_pokaz_ukryj, po);
    }
    apl.uruchom_obserwatora();
    wlacz_autostart(&handle, ust.autostart);

    let dla_systemu = Arc::clone(&apl);
    system::nasluchuj(Box::new(move |z| dla_systemu.zdarzenie_systemowe(z)));

    let dla_monitora = Arc::clone(&apl);
    thread::Builder::new().name("monitor".into()).spawn(move || dla_monitora.petla(sygnaly))?;
    Ok(())
}

/// Autostart po zalogowaniu do Windows. W wersji deweloperskiej pomijany,
/// żeby nie wpisywać do rejestru ścieżki z katalogu `target`.
fn wlacz_autostart(app: &AppHandle, wlacz: bool) {
    if cfg!(debug_assertions) {
        return;
    }
    use tauri_plugin_autostart::ManagerExt;
    let autostart = app.autolaunch();
    let jest = autostart.is_enabled().unwrap_or(false);
    if wlacz && !jest {
        match autostart.enable() {
            Ok(()) => log::info!("autostart włączony"),
            Err(e) => log::warn!("nie mogę włączyć autostartu: {e}"),
        }
    } else if !wlacz && jest {
        match autostart.disable() {
            Ok(()) => log::info!("autostart wyłączony"),
            Err(e) => log::warn!("nie mogę wyłączyć autostartu: {e}"),
        }
    }
}

impl Aplikacja {
    /// Wykonuje działanie na rdzeniu, zapisuje stan i rozsyła skutki:
    /// migawkę i dymki do frontendu, widoczność okna, menu zasobnika.
    pub fn wykonaj<T>(self: &Arc<Self>, dzialanie: impl FnOnce(&mut Rdzen, Chwila) -> T) -> T {
        let teraz = czas::teraz();
        let (wynik, migawka, zdarzenia, menu, ochrona) = {
            let mut r = zablokuj(&self.rdzen);
            let wynik = dzialanie(&mut r, teraz);
            r.zapisz_stan(teraz);
            let zdarzenia = if self.gotowy.load(Ordering::SeqCst) { r.wez_zdarzenia() } else { Vec::new() };
            (wynik, r.migawka(), zdarzenia, stan_menu(&r), r.ust.ukryj_przed_nagrywaniem)
        };
        self.rozeslij(&migawka, zdarzenia);
        if self.gotowy.load(Ordering::SeqCst) {
            self.zastosuj_widocznosc(migawka.widoczna);
        }
        if let Some(zasobnik) = self.zasobnik.get() {
            zasobnik.odswiez(&menu);
        }
        self.zastosuj_ochrone(ochrona);
        self.obudz();
        wynik
    }

    fn rozeslij(&self, migawka: &Migawka, zdarzenia: Vec<Zdarzenie>) {
        if let Ok(json) = serde_json::to_string(migawka) {
            let mut ostatnia = zablokuj(&self.ostatnia_migawka);
            if *ostatnia != json {
                *ostatnia = json;
                drop(ostatnia);
                let _ = self.app.emit("migawka", migawka);
            }
        }
        for zdarzenie in zdarzenia {
            let _ = self.app.emit("zdarzenie", &zdarzenie);
        }
    }

    fn obudz(&self) {
        let _ = zablokuj(&self.budzik).send(());
    }

    pub fn migawka(&self) -> Migawka {
        zablokuj(&self.rdzen).migawka()
    }

    /// Frontend narysował pierwszy stan: ustawiamy okno, pokazujemy je i
    /// wysyłamy dymki zebrane od startu.
    pub fn gotowy(self: &Arc<Self>, szer: f64, wys: f64) {
        zablokuj(&self.okno).rozmiar = Some((szer, wys));
        self.umiesc_okno();
        self.gotowy.store(true, Ordering::SeqCst);
        *zablokuj(&self.ostatnia_migawka) = String::new();
        self.wykonaj(|r, t| r.przesun_powitanie(t));
    }

    pub fn dopasuj(self: &Arc<Self>, szer: f64, wys: f64) {
        zablokuj(&self.okno).rozmiar = Some((szer, wys));
        self.umiesc_okno();
    }

    // ---- okno -----------------------------------------------------------

    /// Monitory systemu. Wszystkie obliczenia położenia są względem monitora
    /// widgetu, a główny służy tylko za miejsce domyślne (B6).
    fn ekrany(&self) -> Vec<Ekran> {
        // Test w wersji deweloperskiej: wirtualny ekran Xvfb ma jeden monitor, więc
        // układ monitorów można podać w pliku i zmieniać w trakcie (odłączenie).
        #[cfg(debug_assertions)]
        if let Some(plik) = std::env::var_os("ASYSTENTKA_MONITORY") {
            return fs::read_to_string(plik).map(|opis| okno::ekrany_z_opisu(&opis)).unwrap_or_default();
        }
        let glowny = self.app.primary_monitor().ok().flatten();
        let mut ekrany: Vec<Ekran> = self
            .app
            .available_monitors()
            .unwrap_or_default()
            .iter()
            .map(|m| {
                let (p, r, w) = (m.position(), m.size(), m.work_area());
                let pelny = Prostokat { x: p.x, y: p.y, szer: r.width, wys: r.height };
                Ekran {
                    nazwa: m.name().cloned().unwrap_or_default(),
                    pelny,
                    obszar: Obszar {
                        x: w.position.x,
                        y: w.position.y,
                        szer: w.size.width,
                        wys: w.size.height,
                        skala: m.scale_factor(),
                    },
                    glowny: glowny.as_ref().is_some_and(|g| (g.position(), g.size()) == (m.position(), m.size())),
                }
            })
            .collect();
        if !ekrany.iter().any(|e| e.glowny) {
            if let Some(pierwszy) = ekrany.first_mut() {
                pierwszy.glowny = true;
            }
        }
        ekrany
    }

    /// Ustawia okno przy kotwicy, na monitorze widgetu. Rozmiar przelicza ze
    /// skalowaniem tego monitora, więc po przejściu na monitor z innym
    /// skalowaniem postać ma od razu tę samą wielkość logiczną.
    fn umiesc_okno(self: &Arc<Self>) {
        let Some((szer, wys)) = zablokuj(&self.okno).rozmiar else { return };
        let zapisana = zablokuj(&self.rdzen).kotwica();
        let ekrany = self.ekrany();
        let Some(polozenie) = okno::polozenie(&zapisana, &ekrany) else { return };
        if polozenie.zastepcze {
            log::info!("monitor widgetu zniknął: staję w domyślnym miejscu na monitorze głównym");
        }
        let (szer, wys) = okno::rozmiar_fizyczny(szer, wys, polozenie.ekran.obszar.skala);
        let cel = okno::umiesc(&polozenie.kotwica, szer, wys, &polozenie.ekran.obszar);
        let zmienione = {
            let mut stan = zablokuj(&self.okno);
            stan.ekran = Some(polozenie.ekran.clone());
            stan.monitory = Some(ekrany);
            if stan.prostokat == Some(cel) {
                false
            } else {
                stan.prostokat = Some(cel);
                true
            }
        };
        // Róg w migawce (odbicie lustrzane, kierunek rozwijania) wynika z monitora widgetu.
        let naroznik = polozenie.kotwica.kotwica;
        if zablokuj(&self.rdzen).naroznik() != naroznik {
            self.wykonaj(|r, _| r.ustaw_naroznik(naroznik));
        }
        if !zmienione {
            return;
        }
        let app = self.app.clone();
        let _ = self.app.run_on_main_thread(move || {
            if let Some(okno) = app.get_webview_window(OKNO) {
                system::ustaw_prostokat(&okno, cel.x, cel.y, cel.szer, cel.wys);
            }
        });
    }

    /// Zmiana zestawu albo układu monitorów (odłączenie, podłączenie,
    /// rozdzielczość, pasek zadań): okno staje na swoim miejscu od nowa.
    fn sprawdz_monitory(self: &Arc<Self>) {
        let ekrany = self.ekrany();
        {
            let mut stan = zablokuj(&self.okno);
            if stan.monitory.as_ref() == Some(&ekrany) {
                return;
            }
            if stan.monitory.is_some() {
                log::info!("zmiana monitorów: {}", ekrany.len());
            }
            stan.monitory = Some(ekrany);
        }
        self.umiesc_okno();
    }

    /// „Przywróć pozycję”: domyślne miejsce na monitorze głównym (B6).
    pub fn przywroc_pozycje(self: &Arc<Self>) {
        log::info!("przywracam pozycję okna");
        self.wykonaj(|r, t| r.przywroc_pozycje(t));
        zablokuj(&self.okno).prostokat = None;
        self.umiesc_okno();
    }

    /// Mousedown na postaci: zaczyna przeciąganie okna przez użytkownika.
    pub fn przeciagnij(&self) -> Result<(), String> {
        zablokuj(&self.okno).reka_od = Some(Instant::now());
        let okno = self.app.get_webview_window(OKNO).ok_or("brak okna widgetu")?;
        okno.start_dragging().map_err(|e| e.to_string())
    }

    fn zastosuj_widocznosc(self: &Arc<Self>, widoczna: bool) {
        let pokolenie = {
            let mut stan = zablokuj(&self.okno);
            if stan.widoczna == Some(widoczna) {
                return;
            }
            stan.widoczna = Some(widoczna);
            stan.pokolenie += 1;
            stan.pokolenie
        };
        log::info!("okno {}", if widoczna { "widoczne" } else { "schowane" });
        if widoczna {
            let app = self.app.clone();
            let _ = self.app.run_on_main_thread(move || {
                if let Some(okno) = app.get_webview_window(OKNO) {
                    system::pokaz_bez_aktywacji(&okno);
                }
            });
            return;
        }
        // Frontend gra glitch wyjścia na podstawie migawki; potem chowamy okno.
        let apl = Arc::clone(self);
        thread::spawn(move || {
            thread::sleep(GLITCH_WYJSCIA);
            if zablokuj(&apl.okno).pokolenie != pokolenie {
                return;
            }
            let app = apl.app.clone();
            let _ = apl.app.run_on_main_thread(move || {
                if let Some(okno) = app.get_webview_window(OKNO) {
                    let _ = okno.hide();
                }
            });
        });
    }

    fn zastosuj_ochrone(&self, wlacz: bool) {
        {
            let mut ochrona = zablokuj(&self.ochrona);
            if *ochrona == Some(wlacz) {
                return;
            }
            *ochrona = Some(wlacz);
        }
        let app = self.app.clone();
        let _ = self.app.run_on_main_thread(move || {
            if let Some(okno) = app.get_webview_window(OKNO) {
                match system::wyklucz_z_przechwytywania(&okno, wlacz) {
                    Ok(()) => log::info!("ukrycie przed nagrywaniem: {}", if wlacz { "włączone" } else { "wyłączone" }),
                    Err(e) => log::warn!("SetWindowDisplayAffinity nie zadziałało: {e}"),
                }
            }
        });
    }

    /// Okno przesunięte. Nasze własne ustawienie ignorujemy. Ruch po kliknięciu
    /// w postać to przeciąganie użytkownika; każdy inny ruch to system (zmiana
    /// monitorów, skalowania), po którym okno wraca przy kotwicy, a zapisana
    /// pozycja zostaje, jaka była.
    pub fn przesunieto(&self, pozycja: PhysicalPosition<i32>) {
        let mut stan = zablokuj(&self.okno);
        let Some(p) = stan.prostokat else { return };
        if (p.x - pozycja.x).abs() <= 2 && (p.y - pozycja.y).abs() <= 2 {
            return;
        }
        let teraz = Instant::now();
        if stan.reka_od.is_some_and(|t| t.elapsed() < RECZNE_PRZESUNIECIE) {
            stan.reka_od = Some(teraz);
            stan.przesuniete = Some(teraz);
            stan.do_poprawy = None;
        } else if okno::przeniesione_przez_system(&p, pozycja.x, pozycja.y) {
            stan.do_poprawy = Some(teraz);
        }
        drop(stan);
        self.obudz();
    }

    /// Zwraca chwilę następnego sprawdzenia, jeśli przeciąganie jeszcze trwa.
    fn zakoncz_przeciaganie(self: &Arc<Self>) -> Option<Instant> {
        let (przesuniete, do_poprawy) = {
            let stan = zablokuj(&self.okno);
            (stan.przesuniete, stan.do_poprawy)
        };
        if let Some(kiedy) = przesuniete {
            if kiedy.elapsed() < PO_PRZECIAGNIECIU {
                return Some(kiedy + PO_PRZECIAGNIECIU);
            }
            {
                let mut stan = zablokuj(&self.okno);
                stan.przesuniete = None;
                stan.reka_od = None;
            }
            let okno = self.app.get_webview_window(OKNO)?;
            let (Ok(pozycja), Ok(rozmiar)) = (okno.outer_position(), okno.outer_size()) else { return None };
            let prostokat = Prostokat { x: pozycja.x, y: pozycja.y, szer: rozmiar.width, wys: rozmiar.height };
            let kotwica = okno::kotwica_po_przeciagnieciu(&prostokat, &self.ekrany())?;
            log::info!("nowa pozycja okna: {kotwica:?}");
            zablokuj(&self.okno).prostokat = Some(prostokat);
            self.wykonaj(|r, t| r.ustaw_kotwice(kotwica, t));
            self.umiesc_okno();
            return None;
        }
        if let Some(kiedy) = do_poprawy {
            if kiedy.elapsed() < PO_PRZECIAGNIECIU {
                return Some(kiedy + PO_PRZECIAGNIECIU);
            }
            {
                let mut stan = zablokuj(&self.okno);
                stan.do_poprawy = None;
                stan.prostokat = None;
            }
            log::info!("system przesunął okno: stawiam je z powrotem przy kotwicy");
            self.umiesc_okno();
        }
        None
    }

    // ---- kursor i pełny ekran -------------------------------------------------

    fn sledz_kursor(&self) -> bool {
        let stan_okna = zablokuj(&self.okno).widoczna == Some(true);
        stan_okna && zablokuj(&self.rdzen).stan() == Stan::Praca
    }

    fn sprawdz_kursor(&self, sledz: bool) {
        let blisko = sledz
            && (|| {
                let (prostokat, skala) = {
                    let stan = zablokuj(&self.okno);
                    (stan.prostokat?, stan.ekran.as_ref()?.obszar.skala)
                };
                let (x, y) =
                    system::kursor().or_else(|| self.app.cursor_position().ok().map(|p| (p.x as i32, p.y as i32)))?;
                Some(okno::odleglosc(&prostokat, x, y) < BLISKO_PX * skala)
            })()
            .unwrap_or(false);
        if self.kursor_blisko.swap(blisko, Ordering::SeqCst) != blisko {
            let _ = self.app.emit("kursor", blisko);
        }
    }

    fn sprawdz_pelny_ekran(self: &Arc<Self>) {
        let wlasne = Some(self.hwnd.load(Ordering::SeqCst)).filter(|h| *h != 0);
        // Pełny ekran liczy się tylko na monitorze widgetu (B5, B6).
        let monitor_widgetu = zablokuj(&self.okno).ekran.as_ref().map(|e| e.pelny);
        let stan = system::pelny_ekran(wlasne);
        let pelny = monitor_widgetu.is_some_and(|m| system::pelny_ekran_na_monitorze(&stan, &m));
        if self.pelny_ekran.swap(pelny, Ordering::SeqCst) != pelny {
            log::info!("pełny ekran: {pelny}");
            self.wykonaj(|r, _| r.ustaw_pelny_ekran(pelny));
        }
    }

    // ---- ustawienia i skrót -------------------------------------------------

    fn sprawdz_ustawienia(self: &Arc<Self>) {
        let sciezka = zablokuj(&self.rdzen).ustawienia_sciezka();
        let zmienione = fs::metadata(&sciezka).and_then(|m| m.modified()).ok();
        {
            let mut ostatnio = zablokuj(&self.ustawienia_zmienione);
            if *ostatnio == zmienione || zmienione.is_none() {
                return;
            }
            *ostatnio = zmienione;
        }
        match ustawienia::wczytaj(&sciezka) {
            Ok(nowe) => {
                log::info!("wczytano zmienione ustawienia");
                let skrot_dziala = self.zarejestruj_skrot(&nowe.skrot_pokaz_ukryj);
                let skrot_tekst = nowe.skrot_pokaz_ukryj.clone();
                self.wykonaj(|r, t| {
                    r.zmien_ustawienia(nowe, t);
                    if !skrot_dziala {
                        r.skrot_zajety(&skrot_tekst, t);
                    }
                });
                self.uruchom_obserwatora();
            }
            Err(e) => {
                log::warn!("{e}; zostają poprzednie ustawienia");
                self.wykonaj(|r, _| r.blad_ustawien());
            }
        }
    }

    /// Obserwuje plik zadań przez powiadomienia systemu plików (bez
    /// odpytywania): zapis z Obsidiana odświeża listę po ułamku sekundy.
    /// Obserwuje folder pliku, bo edytory zapisują przez zamianę nazwy.
    fn uruchom_obserwatora(self: &Arc<Self>) {
        use notify::{EventKind, RecursiveMode, Watcher};
        let plik = zablokuj(&self.rdzen).ust.plik_zadan();
        let mut obserwator = zablokuj(&self.obserwator);
        if obserwator.as_ref().map(|(p, _)| p) == plik.as_ref() {
            return;
        }
        *obserwator = None;
        let Some(plik) = plik else {
            drop(obserwator);
            zablokuj(&self.rdzen).ustaw_obserwacje_pliku(false);
            return;
        };
        let nazwa = plik.file_name().map(std::ffi::OsStr::to_os_string);
        let slaby = Arc::downgrade(self);
        let obserwowany = plik.clone();
        let utworzony = notify::recommended_watcher(move |wynik: notify::Result<notify::Event>| {
            let Ok(zdarzenie) = wynik else { return };
            if matches!(zdarzenie.kind, EventKind::Access(_)) {
                return;
            }
            if !zdarzenie.paths.iter().any(|p| p.file_name() == nazwa.as_deref()) {
                return;
            }
            if let Some(apl) = slaby.upgrade() {
                *zablokuj(&apl.plik_zmieniony) = Some(Instant::now());
                apl.obudz();
            }
        })
        .and_then(|mut w| {
            let folder = obserwowany.parent().filter(|f| !f.as_os_str().is_empty()).unwrap_or(Path::new("."));
            w.watch(folder, RecursiveMode::NonRecursive)?;
            Ok(w)
        });
        let dziala = match utworzony {
            Ok(w) => {
                log::info!("obserwuję plik zadań: {}", plik.display());
                *obserwator = Some((plik, w));
                true
            }
            Err(e) => {
                log::warn!("nie mogę obserwować {}: {e}; odpytuję co 30 s", plik.display());
                false
            }
        };
        drop(obserwator);
        zablokuj(&self.rdzen).ustaw_obserwacje_pliku(dziala);
    }

    /// Rejestruje skrót pokazujący i chowający widget. `false`, gdy się nie udało
    /// (zajmuje go inny program): widget działa dalej bez skrótu i raz o tym mówi (B6).
    fn zarejestruj_skrot(&self, skrot: &str) -> bool {
        *zablokuj(&self.skrot_docelowy) = skrot.trim().to_string();
        self.zastosuj_skrot()
    }

    /// Doprowadza rejestrację w systemie do tego, czego chcą ustawienia, chyba że
    /// pole skrótu właśnie nagrywa kombinację.
    fn zastosuj_skrot(&self) -> bool {
        let docelowy = zablokuj(&self.skrot_docelowy).clone();
        let chce = !docelowy.is_empty() && !self.skrot_wstrzymany.load(Ordering::SeqCst);
        let mut obecny = zablokuj(&self.skrot);
        if chce && obecny.as_deref() == Some(docelowy.as_str()) {
            return true;
        }
        let skroty = self.app.global_shortcut();
        if let Some(stary) = obecny.take() {
            let _ = skroty.unregister(stary.as_str());
        }
        if !chce {
            return true;
        }
        match skroty.register(docelowy.as_str()) {
            Ok(()) => {
                log::info!("skrót pokaż/ukryj: {docelowy}");
                *obecny = Some(docelowy);
                true
            }
            Err(e) => {
                log::warn!("nie mogę zarejestrować skrótu {docelowy}: {e}");
                false
            }
        }
    }

    /// Pole skrótu w ustawieniach nagrywa kombinację: na ten czas zwalniamy własny
    /// skrót, bo inaczej system oddałby naciśnięcie nam, a nie polu.
    pub fn skrot_wstrzymaj(&self, wstrzymaj: bool) {
        self.skrot_wstrzymany.store(wstrzymaj, Ordering::SeqCst);
        self.zastosuj_skrot();
    }

    /// Sprawdza kombinację z pola w ustawieniach: reguły (Ctrl+Alt z literą to AltGr)
    /// i to, czy da się ją zarejestrować. Zwraca zapis kanoniczny.
    pub fn sprawdz_skrot(&self, tekst: &str) -> Result<String, String> {
        let analiza = skrot::analizuj(tekst)?;
        let docelowy = zablokuj(&self.skrot_docelowy).clone();
        let juz_nasz = !self.skrot_wstrzymany.load(Ordering::SeqCst)
            && skrot::analizuj(&docelowy).is_ok_and(|d| d.tekst == analiza.tekst)
            && zablokuj(&self.skrot).is_some();
        if juz_nasz {
            return Ok(analiza.tekst);
        }
        let skroty = self.app.global_shortcut();
        match skroty.register(analiza.skrot) {
            Ok(()) => {
                let _ = skroty.unregister(analiza.skrot);
                Ok(analiza.tekst)
            }
            Err(e) => {
                log::info!("skrót {} jest zajęty: {e}", analiza.tekst);
                Err(format!(
                    "Skrót {} jest zajęty przez inny program. Wybierz inny.",
                    skrot::dla_czlowieka(&analiza.tekst)
                ))
            }
        }
    }

    pub fn skrot(self: &Arc<Self>) {
        self.wykonaj(|r, _| r.przelacz_widocznosc());
    }

    // ---- zasobnik i zdarzenia systemowe ---------------------------------------

    /// Menu zasobnika.
    fn menu(self: &Arc<Self>, zdarzenie: MenuEvent) {
        let id = zdarzenie.id().as_ref();
        if id.starts_with(zasobnik::PREFIKS_KONTEKSTOWEGO) {
            return;
        }
        match id {
            zasobnik::POKAZ => self.wykonaj(|r, _| r.przelacz_widocznosc()),
            zasobnik::PAUZA => self.wykonaj(|r, t| r.przelacz_pauze(t)),
            zasobnik::KONIEC => self.wykonaj(|r, t| {
                r.pokaz();
                r.koniec_pracy(t);
            }),
            zasobnik::PRZYWROC => self.przywroc_pozycje(),
            zasobnik::NAGRYWANIE => self.przelacz_ukrycie_przed_nagrywaniem(),
            zasobnik::USTAWIENIA => self.otworz_okno_ustawien(),
            zasobnik::ZAMKNIJ => self.zakoncz(),
            _ => {}
        }
    }

    /// Prawy przycisk na postaci albo panelu: menu z poleceniami (B6).
    pub fn menu_kontekstowe(self: &Arc<Self>) {
        let (mozliwosci, postac) = {
            let r = zablokuj(&self.rdzen);
            (r.mozliwosci(), r.ust.postac().to_string())
        };
        let app = self.app.clone();
        let _ = self.app.run_on_main_thread(move || {
            let Some(okno) = app.get_webview_window(OKNO) else { return };
            match zasobnik::menu_kontekstowe(&app, &mozliwosci, &postac) {
                Ok(menu) => {
                    if let Err(e) = okno.popup_menu(&menu) {
                        log::warn!("nie mogę pokazać menu pod prawym przyciskiem: {e}");
                    }
                }
                Err(e) => log::warn!("nie mogę zbudować menu pod prawym przyciskiem: {e}"),
            }
        });
    }

    /// Wybór pozycji z menu pod prawym przyciskiem.
    pub fn wybor_z_menu_kontekstowego(self: &Arc<Self>, zdarzenie: &MenuEvent) {
        match zdarzenie.id().as_ref() {
            zasobnik::CTX_KONIEC => self.wykonaj(|r, t| {
                r.pokaz();
                r.koniec_pracy(t);
            }),
            zasobnik::CTX_PLAN => self.wykonaj(|r, _| {
                r.pokaz();
                r.otworz_wybor();
            }),
            zasobnik::CTX_DODAJ => {
                self.wykonaj(|r, _| r.pokaz());
                // Formularz to część widoku, więc prosimy o niego frontend.
                let _ = self.app.emit("akcja", "dodaj_zadanie");
            }
            zasobnik::CTX_PAUZA => self.wykonaj(|r, t| r.przelacz_pauze(t)),
            zasobnik::CTX_UKRYJ => self.wykonaj(|r, _| r.przelacz_widocznosc()),
            zasobnik::CTX_PRZYWROC => self.przywroc_pozycje(),
            zasobnik::CTX_USTAWIENIA => self.otworz_okno_ustawien(),
            zasobnik::CTX_ZAMKNIJ => self.zakoncz(),
            _ => {}
        }
    }

    fn przelacz_ukrycie_przed_nagrywaniem(self: &Arc<Self>) {
        let (sciezka, obecne) = {
            let r = zablokuj(&self.rdzen);
            (r.ustawienia_sciezka(), r.ust.ukryj_przed_nagrywaniem)
        };
        let nowe = self.zasobnik.get().and_then(|z| z.nagrywanie_zaznaczone()).unwrap_or(!obecne);
        if let Err(e) = ustawienia::zapisz_pole(&sciezka, "ukryj_przed_nagrywaniem", serde_json::Value::Bool(nowe)) {
            log::warn!("nie mogę zapisać ustawienia: {e}");
        }
        *zablokuj(&self.ustawienia_zmienione) = fs::metadata(&sciezka).and_then(|m| m.modified()).ok();
        self.wykonaj(|r, t| {
            let zmienione = Ustawienia { ukryj_przed_nagrywaniem: nowe, ..r.ust.clone() };
            r.zmien_ustawienia(zmienione, t);
        });
    }

    // ---- kreator i okno ustawień -----------------------------------------------

    fn katalog_konfiguracji(&self) -> PathBuf {
        self.app.path().config_dir().unwrap_or_default()
    }

    pub fn ustawienia_biezace(&self) -> Ustawienia {
        zablokuj(&self.rdzen).ust.clone()
    }

    pub fn podpowiedz_kreatora(&self) -> PodpowiedzKreatora {
        let vaulty = obsidian::vaulty(&self.katalog_konfiguracji());
        ustawienia::podpowiedz_kreatora(&self.ustawienia_biezace(), &vaulty)
    }

    /// Sprawdza, zapisuje i stosuje ustawienia poza rdzeniem: skrót globalny
    /// i autostart. Przy `utworz_plik` tworzy plik zadań z szablonu (nigdy
    /// nie nadpisuje istniejącego). Zwraca pierwszy błąd w formie komunikatu;
    /// w `Ok` jest informacja, czy skrót udało się zarejestrować (zajęty skrót
    /// nie blokuje zapisu, widget mówi o nim w dymku). Skrót trafia do pliku
    /// w zapisie kanonicznym.
    fn zapisz_i_zastosuj(&self, nowe: &mut Ustawienia, utworz_plik: bool) -> Result<bool, String> {
        let bledy = nowe.waliduj();
        if !bledy.is_empty() {
            return Err(bledy.join("\n"));
        }
        nowe.skrot_pokaz_ukryj = skrot::analizuj(&nowe.skrot_pokaz_ukryj)?.tekst;
        let plik = nowe.plik_zadan().ok_or("Wskaż plik zadań.")?;
        if utworz_plik {
            obsidian::utworz_plik_zadan(&plik, czas::dzien(czas::teraz()), nowe.postac()).map_err(|e| {
                match e.kind() {
                    std::io::ErrorKind::AlreadyExists => {
                        "Ten plik już istnieje. Wybierz „Mam własny plik”, żeby go użyć.".to_string()
                    }
                    std::io::ErrorKind::NotFound => format!("Folder pliku zadań nie istnieje: {}", plik.display()),
                    _ => format!("Nie mogę utworzyć pliku zadań: {e}"),
                }
            })?;
        } else if !plik.is_file() {
            return Err(format!("Nie widzę pliku zadań: {}", plik.display()));
        }
        let folder = nowe.folder_dziennika().ok_or("Wskaż folder dziennika.")?;
        if !folder.is_dir() {
            std::fs::create_dir_all(&folder)
                .map_err(|e| format!("Nie mogę utworzyć folderu dziennika {}: {e}", folder.display()))?;
        }
        let sciezka = zablokuj(&self.rdzen).ustawienia_sciezka();
        ustawienia::zapisz(&sciezka, nowe)?;
        *zablokuj(&self.ustawienia_zmienione) = fs::metadata(&sciezka).and_then(|m| m.modified()).ok();
        let skrot_dziala = self.zarejestruj_skrot(&nowe.skrot_pokaz_ukryj);
        wlacz_autostart(&self.app, nowe.autostart);
        Ok(skrot_dziala)
    }

    /// Okienko ustawień: zmiany działają od razu, bez restartu.
    pub fn zapisz_ustawienia(self: &Arc<Self>, mut nowe: Ustawienia) -> Result<(), String> {
        let skrot_dziala = self.zapisz_i_zastosuj(&mut nowe, false)?;
        let skrot_tekst = nowe.skrot_pokaz_ukryj.clone();
        self.wykonaj(|r, t| {
            r.zmien_ustawienia(nowe, t);
            if !skrot_dziala {
                r.skrot_zajety(&skrot_tekst, t);
            }
        });
        self.uruchom_obserwatora();
        Ok(())
    }

    pub fn zakoncz_kreator(self: &Arc<Self>, mut nowe: Ustawienia, utworz_plik: bool) -> Result<(), String> {
        let skrot_dziala = self.zapisz_i_zastosuj(&mut nowe, utworz_plik)?;
        let plik = nowe.plik_zadan();
        let skrot_tekst = nowe.skrot_pokaz_ukryj.clone();
        self.wykonaj(|r, t| {
            r.zakoncz_kreator(nowe, t);
            if !skrot_dziala {
                r.skrot_zajety(&skrot_tekst, t);
            }
        });
        self.uruchom_obserwatora();
        if let (true, Some(plik)) = (utworz_plik, plik) {
            self.otworz_plik(&plik);
        }
        Ok(())
    }

    /// Plik z vaulta otwiera w Obsidianie, a gdy się nie uda, w domyślnym edytorze.
    pub fn otworz_plik(&self, plik: &Path) {
        let vaulty = obsidian::vaulty(&self.katalog_konfiguracji());
        if obsidian::w_vaulcie(plik, &vaulty) {
            match self.app.opener().open_url(obsidian::adres_otwarcia(plik), None::<&str>) {
                Ok(()) => return,
                Err(e) => log::warn!("nie mogę otworzyć pliku w Obsidianie: {e}"),
            }
        }
        if let Err(e) = self.app.opener().open_path(plik.to_string_lossy(), None::<&str>) {
            log::warn!("nie mogę otworzyć {}: {e}", plik.display());
        }
    }

    /// Okno widgetu domyślnie nie przejmuje fokusu (B6). Pola tekstowe
    /// (kreator, „Dodaj zadanie”) potrzebują klawiatury, więc na czas
    /// wpisywania okno staje się aktywowalne.
    pub fn ustaw_fokus(&self, wlacz: bool) {
        let app = self.app.clone();
        let _ = self.app.run_on_main_thread(move || {
            if let Some(okno) = app.get_webview_window(OKNO) {
                let _ = okno.set_focusable(wlacz);
                if wlacz {
                    let _ = okno.set_focus();
                }
            }
        });
    }

    pub fn otworz_okno_ustawien(self: &Arc<Self>) {
        let app = self.app.clone();
        let tytul = format!("{} · ustawienia", zablokuj(&self.rdzen).ust.postac());
        // Budowanie okna z wątku zdarzeń blokuje się na Windows, więc osobny wątek.
        thread::spawn(move || {
            if let Some(okno) = app.get_webview_window(OKNO_USTAWIEN) {
                let _ = okno.show();
                let _ = okno.unminimize();
                let _ = okno.set_focus();
                return;
            }
            let wynik =
                tauri::WebviewWindowBuilder::new(&app, OKNO_USTAWIEN, tauri::WebviewUrl::App("ustawienia.html".into()))
                    .title(tytul)
                    .inner_size(540.0, 700.0)
                    .min_inner_size(460.0, 520.0)
                    .center()
                    .build();
            match wynik {
                Ok(okno) => {
                    let _ = okno.set_focus();
                }
                Err(e) => log::error!("nie mogę otworzyć okna ustawień: {e}"),
            }
        });
    }

    /// „Zamknij asystentkę”: trwająca sesja zapisuje się jak Przerwij, stan
    /// dnia trafia na dysk i proces się kończy. Dzień zostaje otwarty.
    pub fn zakoncz(self: &Arc<Self>) {
        log::info!("zamykanie");
        self.wykonaj(|r, t| r.zamknij_asystentke(t));
        self.app.exit(0);
    }

    /// Ostatni zapis przy wyjściu, bez rozsyłania czegokolwiek.
    pub fn zapisz_przy_wyjsciu(&self) {
        let teraz = czas::teraz();
        let mut r = zablokuj(&self.rdzen);
        r.pauza_automatyczna(PowodPauzy::Zamkniecie, teraz);
    }

    fn zdarzenie_systemowe(self: &Arc<Self>, zdarzenie: ZdarzenieSystemowe) {
        log::info!("zdarzenie systemowe: {zdarzenie:?}");
        match zdarzenie {
            ZdarzenieSystemowe::Blokada => self.wykonaj(|r, t| r.pauza_automatyczna(PowodPauzy::Blokada, t)),
            ZdarzenieSystemowe::Uspienie => self.wykonaj(|r, t| r.pauza_automatyczna(PowodPauzy::Uspienie, t)),
            ZdarzenieSystemowe::KoniecSesji => self.wykonaj(|r, t| r.pauza_automatyczna(PowodPauzy::Zamkniecie, t)),
            ZdarzenieSystemowe::Odblokowanie | ZdarzenieSystemowe::Wybudzenie => self.wykonaj(|r, t| r.powrot(t)),
        }
    }

    // ---- wątek pilnujący terminów ---------------------------------------------

    fn cykl(self: &Arc<Self>) {
        self.sprawdz_ustawienia();
        let bezczynnosc = system::sekundy_bezczynnosci();
        self.wykonaj(|r, t| {
            if let Some(sekundy) = bezczynnosc {
                r.bezczynnosc(sekundy, t);
            }
            r.cykl(t);
        });
        // Poprawka po zmianie rozdzielczości albo paska zadań.
        self.umiesc_okno();
    }

    /// Stan systemu sprawdzany co 2–30 s, kursor co 500 ms tylko w Pracy;
    /// między terminami wątek śpi.
    fn petla(self: Arc<Self>, sygnaly: Receiver<()>) {
        let start = Instant::now();
        let mut cykl = start + CO_ILE_CYKL;
        let mut pelny_ekran = start;
        let mut kursor = start;
        let mut ostatni_obrot = czas::teraz();
        loop {
            let teraz = czas::teraz();
            // Uśpienie bez zdarzenia z Windows (albo zamrożony proces): pauza od ostatniego obrotu.
            if teraz - ostatni_obrot > chrono::Duration::seconds(90) {
                log::info!("przerwa w działaniu od {}", czas::rfc3339(ostatni_obrot));
                let od = ostatni_obrot;
                self.wykonaj(|r, t| {
                    r.pauza_automatyczna(PowodPauzy::Uspienie, od);
                    r.powrot(t);
                });
            }
            ostatni_obrot = teraz;

            let chwila = Instant::now();
            if chwila >= pelny_ekran {
                self.sprawdz_monitory();
                self.sprawdz_pelny_ekran();
                pelny_ekran = chwila + CO_ILE_PELNY_EKRAN;
            }
            if chwila >= cykl {
                self.cykl();
                cykl = chwila + CO_ILE_CYKL;
            }
            let termin_rdzenia = zablokuj(&self.rdzen).najblizszy_termin(teraz);
            if termin_rdzenia <= czas::teraz() {
                self.wykonaj(|r, t| r.tick(t));
            }
            let sledz = self.sledz_kursor();
            if !sledz || chwila >= kursor {
                self.sprawdz_kursor(sledz);
                kursor = chwila + CO_ILE_KURSOR;
            }

            let mut nastepny = pelny_ekran.min(cykl);
            // Zapisy pliku zadań często przychodzą seriami, więc odświeżamy po 300 ms spokoju.
            let zmiana_pliku = *zablokuj(&self.plik_zmieniony);
            if let Some(od) = zmiana_pliku {
                if od.elapsed() >= ODSTEP_PO_ZAPISIE {
                    *zablokuj(&self.plik_zmieniony) = None;
                    self.wykonaj(|r, _| {
                        r.odswiez_zadania();
                    });
                } else {
                    nastepny = nastepny.min(od + ODSTEP_PO_ZAPISIE);
                }
            }
            if sledz {
                nastepny = nastepny.min(kursor);
            }
            if let Some(przeciaganie) = self.zakoncz_przeciaganie() {
                nastepny = nastepny.min(przeciaganie);
            }
            // Sygnały z naszych własnych wywołań `wykonaj` nie mają budzić pętli
            // drugi raz; terminy liczymy po ich odebraniu, więc nic nie ginie.
            while sygnaly.try_recv().is_ok() {}
            let teraz = czas::teraz();
            let termin_rdzenia = zablokuj(&self.rdzen).najblizszy_termin(teraz);
            let do_terminu = (termin_rdzenia - teraz).to_std().unwrap_or_default();
            nastepny = nastepny.min(Instant::now() + do_terminu);
            let sen = nastepny.saturating_duration_since(Instant::now()).max(Duration::from_millis(20));
            match sygnaly.recv_timeout(sen) {
                Ok(()) | Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
    }
}
