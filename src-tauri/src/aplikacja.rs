//! Spina rdzeń z Tauri: zdarzenia dla frontendu, okno w rogu ekranu,
//! zasobnik, skrót globalny, zdarzenia Windows i wątek pilnujący terminów.
//!
//! Zasada: nie wołamy API okna ani menu, trzymając blokadę rdzenia, bo te
//! wywołania mogą czekać na wątek główny, który czeka na tę blokadę.

use std::error::Error;
use std::fs;
use std::path::PathBuf;
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
use crate::okno::{self, Obszar, Prostokat};
use crate::rdzen::{Migawka, Rdzen, Stan, Zdarzenie};
use crate::sesja::PowodPauzy;
use crate::system::{self, ZdarzenieSystemowe};
use crate::ustawienia::{self, Ustawienia};
use crate::zasobnik::{self, StanMenu, Zasobnik};

pub const OKNO: &str = "main";

const CO_ILE_CYKL: Duration = Duration::from_secs(30);
const CO_ILE_PELNY_EKRAN: Duration = Duration::from_secs(2);
const CO_ILE_KURSOR: Duration = Duration::from_millis(500);
const PO_PRZECIAGNIECIU: Duration = Duration::from_millis(400);
/// Glitch wyjścia trwa 400 ms; okno chowamy, gdy się skończy.
const GLITCH_WYJSCIA: Duration = Duration::from_millis(420);
/// Odległość kursora, przy której postać patrzy na Ciebie (piksele logiczne).
const BLISKO_PX: f64 = 300.0;

#[derive(Default)]
struct StanOkna {
    rozmiar: Option<(f64, f64)>,
    prostokat: Option<Prostokat>,
    widoczna: Option<bool>,
    pokolenie: u64,
    przesuniete: Option<Instant>,
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
    skrot: Mutex<Option<String>>,
    ustawienia_zmienione: Mutex<Option<SystemTime>>,
    pelny_ekran: AtomicBool,
    kursor_blisko: AtomicBool,
    ochrona: Mutex<Option<bool>>,
}

fn zablokuj<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn stan_menu(r: &Rdzen) -> StanMenu {
    StanMenu {
        widoczna: r.widoczna(),
        pauza: r.lok.sesja.as_ref().map(|s| s.pauza.is_some()),
        ukryj_przed_nagrywaniem: r.ust.ukryj_przed_nagrywaniem,
    }
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
    let (ust, wynik) = ustawienia::wczytaj_lub_utworz(&sciezka_ustawien);
    let blad_ustawien = match wynik {
        ustawienia::Wynik::Utworzone => {
            log::info!("utworzono {}", sciezka_ustawien.display());
            false
        }
        ustawienia::Wynik::Wczytane => false,
        ustawienia::Wynik::Blad(e) => {
            log::warn!("{e}; używam wartości domyślnych");
            true
        }
    };

    let teraz = czas::teraz();
    let mut rdzen = Rdzen::nowy(magazyn, ust.clone(), teraz);
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
        ustawienia_zmienione: Mutex::new(fs::metadata(&sciezka_ustawien).and_then(|m| m.modified()).ok()),
        pelny_ekran: AtomicBool::new(false),
        kursor_blisko: AtomicBool::new(false),
        ochrona: Mutex::new(None),
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

    apl.zarejestruj_skrot(&ust.skrot_pokaz_ukryj);
    wlacz_autostart(&handle);

    let dla_systemu = Arc::clone(&apl);
    system::nasluchuj(Box::new(move |z| dla_systemu.zdarzenie_systemowe(z)));

    let dla_monitora = Arc::clone(&apl);
    thread::Builder::new().name("monitor".into()).spawn(move || dla_monitora.petla(sygnaly))?;
    Ok(())
}

/// Autostart po zalogowaniu do Windows. W wersji deweloperskiej pomijany,
/// żeby nie wpisywać do rejestru ścieżki z katalogu `target`.
fn wlacz_autostart(app: &AppHandle) {
    if cfg!(debug_assertions) {
        return;
    }
    use tauri_plugin_autostart::ManagerExt;
    let autostart = app.autolaunch();
    if !autostart.is_enabled().unwrap_or(false) {
        match autostart.enable() {
            Ok(()) => log::info!("autostart włączony"),
            Err(e) => log::warn!("nie mogę włączyć autostartu: {e}"),
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

    fn obszar_roboczy(&self) -> Option<Obszar> {
        let monitor = match self.app.primary_monitor() {
            Ok(Some(m)) => m,
            _ => self.app.available_monitors().ok()?.into_iter().next()?,
        };
        let obszar = monitor.work_area();
        Some(Obszar {
            x: obszar.position.x,
            y: obszar.position.y,
            szer: obszar.size.width,
            wys: obszar.size.height,
            skala: monitor.scale_factor(),
        })
    }

    fn umiesc_okno(&self) {
        let Some((szer, wys)) = zablokuj(&self.okno).rozmiar else { return };
        let kotwica = zablokuj(&self.rdzen).lok.okno.clone();
        let Some(obszar) = self.obszar_roboczy() else { return };
        let (szer, wys) = okno::rozmiar_fizyczny(szer, wys, obszar.skala);
        let cel = okno::umiesc(&kotwica, szer, wys, &obszar);
        {
            let mut stan = zablokuj(&self.okno);
            if stan.prostokat == Some(cel) {
                return;
            }
            stan.prostokat = Some(cel);
        }
        let app = self.app.clone();
        let _ = self.app.run_on_main_thread(move || {
            if let Some(okno) = app.get_webview_window(OKNO) {
                system::ustaw_prostokat(&okno, cel.x, cel.y, cel.szer, cel.wys);
            }
        });
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

    /// Okno przesunięte: nasze własne ustawienie ignorujemy, przeciągnięcie
    /// przez użytkownika kończymy po chwili spokoju.
    pub fn przesunieto(&self, pozycja: PhysicalPosition<i32>) {
        let mut stan = zablokuj(&self.okno);
        if let Some(p) = stan.prostokat {
            if (p.x - pozycja.x).abs() <= 2 && (p.y - pozycja.y).abs() <= 2 {
                return;
            }
        }
        stan.przesuniete = Some(Instant::now());
        drop(stan);
        self.obudz();
    }

    /// Zwraca chwilę następnego sprawdzenia, jeśli przeciąganie jeszcze trwa.
    fn zakoncz_przeciaganie(self: &Arc<Self>) -> Option<Instant> {
        let kiedy = zablokuj(&self.okno).przesuniete?;
        if kiedy.elapsed() < PO_PRZECIAGNIECIU {
            return Some(kiedy + PO_PRZECIAGNIECIU);
        }
        zablokuj(&self.okno).przesuniete = None;
        let okno = self.app.get_webview_window(OKNO)?;
        let (Ok(pozycja), Ok(rozmiar)) = (okno.outer_position(), okno.outer_size()) else { return None };
        let obszar = self.obszar_roboczy()?;
        let prostokat = Prostokat { x: pozycja.x, y: pozycja.y, szer: rozmiar.width, wys: rozmiar.height };
        let kotwica = okno::kotwica(&prostokat, &obszar);
        log::info!("nowa pozycja okna: {kotwica:?}");
        zablokuj(&self.okno).prostokat = Some(prostokat);
        self.wykonaj(|r, t| r.ustaw_kotwice(kotwica, t));
        self.umiesc_okno();
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
                let prostokat = zablokuj(&self.okno).prostokat?;
                let skala = self.obszar_roboczy()?.skala;
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
        let pelny = system::pelny_ekran(wlasne);
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
                self.zarejestruj_skrot(&nowe.skrot_pokaz_ukryj);
                self.wykonaj(|r, t| r.zmien_ustawienia(nowe, t));
            }
            Err(e) => {
                log::warn!("{e}; zostają poprzednie ustawienia");
                self.wykonaj(|r, _| r.blad_ustawien());
            }
        }
    }

    fn zarejestruj_skrot(&self, skrot: &str) {
        let mut obecny = zablokuj(&self.skrot);
        if obecny.as_deref() == Some(skrot) {
            return;
        }
        let skroty = self.app.global_shortcut();
        if let Some(stary) = obecny.take() {
            let _ = skroty.unregister(stary.as_str());
        }
        match skroty.register(skrot) {
            Ok(()) => {
                log::info!("skrót pokaż/ukryj: {skrot}");
                *obecny = Some(skrot.to_string());
            }
            Err(e) => log::warn!("nie mogę zarejestrować skrótu {skrot}: {e}"),
        }
    }

    pub fn skrot(self: &Arc<Self>) {
        self.wykonaj(|r, _| r.przelacz_widocznosc());
    }

    // ---- zasobnik i zdarzenia systemowe ---------------------------------------

    fn menu(self: &Arc<Self>, zdarzenie: MenuEvent) {
        match zdarzenie.id().as_ref() {
            zasobnik::POKAZ => self.wykonaj(|r, _| r.przelacz_widocznosc()),
            zasobnik::PAUZA => self.wykonaj(|r, t| r.przelacz_pauze(t)),
            zasobnik::NAGRYWANIE => self.przelacz_ukrycie_przed_nagrywaniem(),
            zasobnik::USTAWIENIA => self.otworz_ustawienia(),
            zasobnik::ZAKONCZ => self.zakoncz(),
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

    fn otworz_ustawienia(&self) {
        let sciezka = zablokuj(&self.rdzen).ustawienia_sciezka();
        if let Err(e) = self.app.opener().open_path(sciezka.to_string_lossy(), None::<&str>) {
            log::warn!("nie mogę otworzyć {}: {e}", sciezka.display());
        }
    }

    pub fn zakoncz(self: &Arc<Self>) {
        log::info!("zamykanie");
        self.wykonaj(|r, t| r.pauza_automatyczna(PowodPauzy::Zamkniecie, t));
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
