//! Dźwięk skupienia (SPEC.md, B7): zapętlone dudnienie 40 Hz grane w Rust przez `rodio`.
//! Nie gramy w WebView2, bo jego blokada autoodtwarzania potrafi zatrzymać dźwięk bez kliknięcia.
//!
//! Cztery warstwy:
//! - [`Sterownik`] ustala, jak głośność ma się zmieniać: narastanie 3 s, cichnięcie 1,5 s,
//!   odsłuch 5 s;
//! - [`Maszyna`] pilnuje urządzenia: otwiera je dopiero wtedy, gdy jest co grać, zamyka po
//!   dłuższej ciszy i ponawia próbę, gdy urządzenia brak albo zniknęło (wypięte słuchawki);
//! - [`Wyjscie`] to urządzenie audio; naprawdę gra tylko [`Rodio`], które rampę głośności
//!   liczy przy każdej próbce (`Rampa`), od pierwszej, jaką odtworzy;
//! - [`Silnik`] to wątek z kolejką poleceń i jedyny uchwyt dla reszty aplikacji.
//!
//! Dwie pierwsze warstwy nie dotykają ani urządzenia, ani zegara, więc testy sprawdzają je
//! sztucznym czasem i atrapą urządzenia.

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use rodio::buffer::SamplesBuffer;
use rodio::cpal::traits::{DeviceTrait, HostTrait};
use rodio::cpal::BufferSize;
use rodio::{CpalError, CpalErrorKind, Decoder, DeviceSinkBuilder, DeviceSinkError, MixerDeviceSink, Player, Source};

/// Dźwięk z B7 wbudowany w program: lewy kanał 200 Hz, prawy 240 Hz, 10 s, 48 kHz, 16 bit.
const WBUDOWANY: &[u8] = include_bytes!("../../src/assets/dzwiek/skupienie_40hz.wav");

/// Własny dźwięk zamiast wbudowanego: pierwszy istniejący plik z tej listy, w katalogu danych.
pub const WLASNE_PLIKI: [&str; 2] = ["skupienie.wav", "skupienie.mp3"];

/// Po starcie i wznowieniu głośność narasta tyle czasu.
pub const NARASTANIE: Duration = Duration::from_secs(3);
/// Po Pauzie, Przerwij, Zrobione, końcu dnia, blokadzie, uśpieniu i w „Jesteś?” cichnie tyle czasu.
pub const CICHNIECIE: Duration = Duration::from_millis(1500);
/// „Odsłuchaj” w ustawieniach gra tyle czasu.
pub const ODSLUCH: Duration = Duration::from_secs(5);
/// Odsłuch jest krótki, więc głośność dochodzi do poziomu z suwaka szybciej niż w pracy.
const ODSLUCH_NARASTANIE: Duration = Duration::from_millis(400);
/// Przy zamykaniu programu dźwięk cichnie szybko, żeby nie urwać go z trzaskiem.
const CICHNIECIE_PRZY_WYJSCIU: Duration = Duration::from_millis(120);
/// Bufor urządzenia: większy od domyślnych 50 ms, bo to tło, a opóźnienie nie ma znaczenia,
/// za to przy obciążonym komputerze (gra i nagrywanie w OBS) rzadziej trzeszczy.
const BUFOR_URZADZENIA: Duration = Duration::from_millis(100);
/// Urządzenie ma w buforze jeszcze 100 ms dźwięku sprzed ciszy; zamknięte wcześniej urwałoby
/// go w połowie fali.
const OPROZNIENIE_BUFORA: Duration = Duration::from_millis(150);
/// Przy bardzo cichym starcie zmiana i tak trwa prawie tyle, ile zwykle, a nie ułamek sekundy.
const NAJNIZSZY_POZIOM_ZMIANY: f32 = 0.02;
/// Różnica głośności poniżej −80 dB jest niesłyszalna: uznajemy cel za osiągnięty.
const NIESLYSZALNA_ROZNICA: f32 = 1e-4;

/// Co ile wątek się budzi w trakcie narastania i cichnięcia (rampę wygładza urządzenie).
pub const KROK: Duration = Duration::from_millis(20);
/// Brak urządzenia audio: ponawiamy próbę co tyle.
const PONOWIENIE_OTWARCIA: Duration = Duration::from_secs(5);
/// Urządzenie zostaje otwarte tyle po wyciszeniu: Pauza i Wznów nie budzą go co chwilę.
const ZAMKNIECIE_PO_CISZY: Duration = Duration::from_secs(30);
/// Grając, wątek co jakiś czas sprawdza, czy urządzenie nie zniknęło.
const SPRAWDZANIE_URZADZENIA: Duration = Duration::from_secs(5);
/// Zmiana głośności po przerwie dłuższej niż to (uśpienie komputera) nie przeskakuje: liczy się
/// jak jeden krok tej długości. Terminy (odsłuch, zamknięcie urządzenia) liczą prawdziwy czas.
const NAJDLUZSZY_KROK_GLOSNOSCI: Duration = Duration::from_millis(250);
/// Tyle odsłuch i zatrzymanie czekają na wątek dźwięku.
const CZEKANIE_NA_WATEK: Duration = Duration::from_secs(4);

// ---- głośność w czasie ----------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
struct Odsluch {
    do_chwili: Duration,
    poziom: f32,
}

/// Głośność od 0 do 1 jako funkcja czasu. Czas to suma kroków, bez zegara systemowego.
///
/// Narastanie z ciszy do poziomu trwa 3 s, cichnięcie do ciszy 1,5 s, niezależnie od poziomu.
/// Tempo zmiany jest stałe od chwili zmiany celu, więc głośność zmienia się liniowo.
/// Zmiana poziomu w trakcie gry (suwak) przesuwa głośność w tym samym tempie.
#[derive(Debug, Clone, Default)]
pub struct Sterownik {
    czas: Duration,
    cel: f32,
    odsluch: Option<Odsluch>,
    glosnosc: f32,
    /// Cel, dla którego policzono `nachylenie`; `None` wymusza przeliczenie.
    zmierza_do: Option<f32>,
    /// Zmiana głośności na sekundę.
    nachylenie: f32,
}

impl Sterownik {
    /// Poziom, do którego ma dojść głośność w pracy; `None` oznacza ciszę.
    /// Zwraca `true`, gdy cel się zmienił.
    pub fn ustaw_cel(&mut self, cel: Option<f32>) -> bool {
        let nowy = cel.map_or(0.0, |c| c.clamp(0.0, 1.0));
        let zmiana = nowy != self.cel;
        self.cel = nowy;
        zmiana
    }

    /// Odsłuch: przez 5 s gra na podanym poziomie, potem wraca do celu z pracy.
    pub fn odsluchaj(&mut self, poziom: f32) {
        self.odsluch = Some(Odsluch { do_chwili: self.czas + ODSLUCH, poziom: poziom.clamp(0.0, 1.0) });
    }

    /// Zamykanie programu: cisza w 120 ms, bez odsłuchu.
    pub fn wycisz_szybko(&mut self) {
        self.cel = 0.0;
        self.odsluch = None;
        self.zmierza_do = Some(0.0);
        self.nachylenie = self.glosnosc.max(NAJNIZSZY_POZIOM_ZMIANY) / CICHNIECIE_PRZY_WYJSCIU.as_secs_f32();
    }

    /// Urządzenie właśnie się otworzyło: gramy od ciszy, żeby narastanie było słyszalne.
    pub fn zeruj(&mut self) {
        self.glosnosc = 0.0;
        self.zmierza_do = None;
    }

    /// Poziom, do którego zmierza głośność: z odsłuchu albo z pracy.
    fn poziom_docelowy(&self) -> f32 {
        self.odsluch.map_or(self.cel, |o| o.poziom)
    }

    /// Zmiana głośności na sekundę, ustalona przy ostatniej zmianie celu.
    fn nachylenie(&self) -> f32 {
        self.nachylenie
    }

    /// Czy w tej chwili jest co grać (praca z dźwiękiem albo odsłuch).
    pub fn chce_grac(&self) -> bool {
        self.poziom_docelowy() > 0.0
    }

    #[cfg(test)]
    pub fn glosnosc(&self) -> f32 {
        self.glosnosc
    }

    pub fn trwa_zmiana(&self) -> bool {
        (self.glosnosc - self.poziom_docelowy()).abs() > NIESLYSZALNA_ROZNICA
    }

    /// Cisza: nic nie gra i nic nie ma zacząć.
    pub fn cichy(&self) -> bool {
        self.glosnosc <= 0.0 && !self.chce_grac()
    }

    /// Za ile skończy się odsłuch.
    fn do_konca_odsluchu(&self) -> Option<Duration> {
        self.odsluch.map(|o| o.do_chwili.saturating_sub(self.czas))
    }

    /// Przesuwa czas o `dt` i zwraca głośność na koniec kroku. Odsłuch, który kończy się w
    /// środku kroku, zostaje w nim rozliczony: do jego końca głośność zmierza do poziomu
    /// odsłuchu, a resztę kroku do celu z pracy.
    pub fn krok(&mut self, dt: Duration) -> f32 {
        if let Some(do_konca) = self.do_konca_odsluchu().filter(|d| *d < dt) {
            self.przesun(do_konca);
            self.odsluch = None;
            return self.przesun(dt - do_konca);
        }
        self.przesun(dt)
    }

    fn przesun(&mut self, dt: Duration) -> f32 {
        self.czas += dt;
        let dt = dt.min(NAJDLUZSZY_KROK_GLOSNOSCI);
        let cel = self.poziom_docelowy();
        if self.zmierza_do != Some(cel) {
            self.zmierza_do = Some(cel);
            let (poziom, czas) = if cel > self.glosnosc {
                (cel, if self.odsluch.is_some() { ODSLUCH_NARASTANIE } else { NARASTANIE })
            } else {
                (self.glosnosc, CICHNIECIE)
            };
            self.nachylenie = poziom.max(NAJNIZSZY_POZIOM_ZMIANY) / czas.as_secs_f32();
        }
        let zmiana = self.nachylenie * dt.as_secs_f32();
        self.glosnosc =
            if cel > self.glosnosc { (self.glosnosc + zmiana).min(cel) } else { (self.glosnosc - zmiana).max(cel) };
        // Błąd zaokrągleń f32 nie może zostawić głośności o włos od celu (np. od ciszy).
        if (self.glosnosc - cel).abs() <= NIESLYSZALNA_ROZNICA {
            self.glosnosc = cel;
        }
        self.glosnosc
    }
}

// ---- urządzenie i maszyna stanów ------------------------------------------------------------

/// Urządzenie audio. Otwarte gra zapętlony dźwięk i słucha tylko głośności.
pub trait Wyjscie {
    /// Otwiera urządzenie i zaczyna grać w pętli z głośnością 0.
    fn otworz(&mut self) -> Result<(), String>;
    fn zamknij(&mut self);
    fn otwarte(&self) -> bool;
    /// Urządzenie, które było otwarte, zniknęło (np. wypięte słuchawki).
    fn utracone(&self) -> bool;
    /// Poziom, do którego urządzenie ma doprowadzić głośność (0 do 1), i zmiana na sekundę.
    /// Urządzenie rozkłada ją na próbki samo, od swojej pierwszej próbki, bo gra zaczyna się
    /// z opóźnieniem po otwarciu i rampa liczona w wątku nie może go wyprzedzać.
    fn ustaw(&mut self, cel: f32, nachylenie: f32);
}

/// Co maszyna chce przekazać wątkowi po kroku.
#[derive(Debug, Default, PartialEq)]
pub struct Wynik {
    /// Nowy stan dostępności urządzenia, gdy się zmienił od ostatniego zgłoszenia.
    pub dostepnosc: Option<bool>,
    /// Za ile wołać `krok` następnym razem; `None`, gdy nic nie czeka na czas.
    pub nastepny: Option<Duration>,
}

pub struct Maszyna<W: Wyjscie> {
    wyjscie: W,
    sterownik: Sterownik,
    czas: Duration,
    nastepna_proba: Duration,
    cisza_od: Option<Duration>,
    zgloszona: Option<bool>,
    nowa_dostepnosc: Option<bool>,
}

impl<W: Wyjscie> Maszyna<W> {
    pub fn nowa(wyjscie: W) -> Self {
        Maszyna {
            wyjscie,
            sterownik: Sterownik::default(),
            czas: Duration::ZERO,
            nastepna_proba: Duration::ZERO,
            cisza_od: None,
            zgloszona: None,
            nowa_dostepnosc: None,
        }
    }

    /// Cel z rdzenia: poziom albo `None` (cisza).
    pub fn cel(&mut self, cel: Option<f32>) {
        // Rdzeń powtarza ten sam cel przy każdej akcji; ponawianie otwarcia nie ma wtedy ruszać.
        if self.sterownik.ustaw_cel(cel) {
            self.nastepna_proba = self.czas;
        }
    }

    /// „Odsłuchaj”: gra 5 s na podanym poziomie. Odmawia, gdy nie ma urządzenia.
    pub fn odsluch(&mut self, poziom: f32) -> Result<(), String> {
        if poziom <= 0.0 {
            return Err("Głośność jest ustawiona na 0, więc nic nie usłyszysz.".into());
        }
        if self.wyjscie.otwarte() && self.wyjscie.utracone() {
            self.wyjscie.zamknij();
        }
        if !self.wyjscie.otwarte() {
            self.otworz()?;
        }
        self.sterownik.odsluchaj(poziom);
        self.cisza_od = None;
        Ok(())
    }

    /// Zamykanie programu: szybka cisza, potem zamknięcie urządzenia po opróżnieniu jego
    /// bufora. `czekaj` czeka podany czas; wątek podaje `thread::sleep`.
    pub fn zatrzymaj(&mut self, mut czekaj: impl FnMut(Duration)) {
        if self.wyjscie.otwarte() {
            self.sterownik.wycisz_szybko();
            while !self.sterownik.cichy() {
                czekaj(KROK);
                self.krok(KROK);
            }
            czekaj(OPROZNIENIE_BUFORA);
        }
        self.wyjscie.zamknij();
    }

    fn otworz(&mut self) -> Result<(), String> {
        match self.wyjscie.otworz() {
            Ok(()) => {
                self.sterownik.zeruj();
                self.cisza_od = None;
                self.zglos(true);
                Ok(())
            }
            Err(e) => {
                self.nastepna_proba = self.czas + PONOWIENIE_OTWARCIA;
                // Brak urządzenia może trwać godzinami; do logu trafia tylko pierwsza próba.
                if self.zgloszona != Some(false) {
                    log::warn!("dźwięk skupienia: {e}");
                }
                self.zglos(false);
                Err(e)
            }
        }
    }

    fn zglos(&mut self, dostepne: bool) {
        if self.zgloszona != Some(dostepne) {
            self.zgloszona = Some(dostepne);
            self.nowa_dostepnosc = Some(dostepne);
        }
    }

    /// Przesuwa czas o `dt` (tyle, ile wątek faktycznie czekał) i dopasowuje urządzenie do celu.
    pub fn krok(&mut self, dt: Duration) -> Wynik {
        self.czas += dt;
        if self.wyjscie.otwarte() && self.wyjscie.utracone() {
            log::warn!("urządzenie audio zniknęło; otwieram je od nowa przy następnej okazji");
            self.wyjscie.zamknij();
            self.nastepna_proba = self.czas;
        }
        if self.sterownik.chce_grac() && !self.wyjscie.otwarte() && self.czas >= self.nastepna_proba {
            let _ = self.otworz();
        }
        self.sterownik.krok(dt);
        if self.wyjscie.otwarte() {
            self.wyjscie.ustaw(self.sterownik.poziom_docelowy(), self.sterownik.nachylenie());
            if self.sterownik.cichy() {
                let od = *self.cisza_od.get_or_insert(self.czas);
                if self.czas - od >= ZAMKNIECIE_PO_CISZY {
                    self.wyjscie.zamknij();
                    self.cisza_od = None;
                }
            } else {
                self.cisza_od = None;
            }
        } else {
            // Bez urządzenia nic nie narasta: dźwięk zacznie się od ciszy, gdy urządzenie wróci.
            self.sterownik.zeruj();
        }
        Wynik { dostepnosc: self.nowa_dostepnosc.take(), nastepny: self.kiedy_nastepny() }
    }

    /// Głośność, którą wątek uważa za bieżącą (urządzenie liczy własną rampę).
    #[cfg(test)]
    fn glosnosc(&self) -> f32 {
        self.sterownik.glosnosc()
    }

    /// Polecenie przychodzi w trakcie snu wątku: najpierw nadrabiamy czas, który upłynął od
    /// ostatniego obrotu (inaczej odsłuch liczyłby swoje 5 s od starej chwili), potem
    /// wykonujemy polecenie i na koniec układamy nowy harmonogram.
    pub fn obsluz(&mut self, dt: Duration, polecenie: impl FnOnce(&mut Self)) -> Wynik {
        let przed = self.krok(dt);
        polecenie(self);
        let po = self.krok(Duration::ZERO);
        Wynik { dostepnosc: po.dostepnosc.or(przed.dostepnosc), nastepny: po.nastepny }
    }

    fn kiedy_nastepny(&self) -> Option<Duration> {
        let mut terminy: Vec<Duration> = Vec::new();
        if self.wyjscie.otwarte() {
            if self.sterownik.trwa_zmiana() {
                terminy.push(KROK);
            }
            if let Some(do_konca) = self.sterownik.do_konca_odsluchu() {
                terminy.push(do_konca.max(KROK));
            }
            match self.cisza_od {
                Some(od) => terminy.push((od + ZAMKNIECIE_PO_CISZY).saturating_sub(self.czas).max(KROK)),
                None => terminy.push(SPRAWDZANIE_URZADZENIA),
            }
        } else if self.sterownik.chce_grac() {
            terminy.push(self.nastepna_proba.saturating_sub(self.czas).max(KROK));
        }
        terminy.into_iter().min()
    }
}

// ---- własny plik, wbudowany plik i urządzenie rodio -------------------------------------------

/// Plik, który gra zamiast wbudowanego dźwięku: `skupienie.wav`, a w jego braku `skupienie.mp3`.
pub fn wlasny_plik(katalog: &Path) -> Option<PathBuf> {
    WLASNE_PLIKI.iter().map(|nazwa| katalog.join(nazwa)).find(|p| p.is_file())
}

/// Dekoduje wbudowany plik do próbek: pętla z `repeat_infinite()` łączy się bez przerwy.
fn wbudowany() -> Result<SamplesBuffer, String> {
    let dekoder = Decoder::new_wav(std::io::Cursor::new(WBUDOWANY)).map_err(|e| format!("wbudowany dźwięk: {e}"))?;
    let (kanaly, czestotliwosc) = (dekoder.channels(), dekoder.sample_rate());
    Ok(SamplesBuffer::new(kanaly, czestotliwosc, dekoder.collect::<Vec<_>>()))
}

/// Własny plik gra strumieniowo w pętli, bo godzinny mp3 nie zmieści się w pamięci jako próbki.
fn z_pliku(plik: &Path) -> Result<Box<dyn Source + Send>, String> {
    let otwarty = File::open(plik).map_err(|e| e.to_string())?;
    let dlugosc = otwarty.metadata().map_err(|e| e.to_string())?.len();
    let rozszerzenie = plik.extension().and_then(|r| r.to_str()).unwrap_or_default().to_lowercase();
    let dekoder = Decoder::builder()
        .with_data(BufReader::new(otwarty))
        .with_byte_len(dlugosc)
        .with_hint(&rozszerzenie)
        .build_looped()
        .map_err(|e| e.to_string())?;
    Ok(Box::new(dekoder))
}

/// Dźwięk do zapętlenia: własny plik, jeśli leży w katalogu danych i da się go odczytać,
/// a inaczej wbudowany.
fn zrodlo_dzwieku(
    katalog: Option<&Path>,
    wbudowany_cache: &mut Option<SamplesBuffer>,
) -> Result<Box<dyn Source + Send>, String> {
    if let Some(plik) = katalog.and_then(wlasny_plik) {
        match z_pliku(&plik) {
            Ok(zrodlo) => {
                log::info!("dźwięk skupienia z pliku {}", plik.display());
                return Ok(zrodlo);
            }
            Err(e) => log::warn!("nie mogę zagrać {}: {e}; używam wbudowanego dźwięku", plik.display()),
        }
    }
    let dzwiek = match wbudowany_cache {
        Some(rozkodowany) => rozkodowany.clone(),
        None => wbudowany_cache.insert(wbudowany()?).clone(),
    };
    Ok(Box::new(dzwiek.repeat_infinite()))
}

/// Cel i tempo zmiany głośności: wątek pisze, wątek audio czyta.
#[derive(Debug, Default)]
struct Sterowanie {
    cel: AtomicU32,
    nachylenie: AtomicU32,
}

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

/// Nakładka na dźwięk, która zmienia głośność liniowo przy każdej próbce, od pierwszej, jaką
/// urządzenie odtworzy. Gdyby rampę liczył tylko wątek (co 20 ms), dźwięk miałby schodki
/// słyszalne na czystym tonie, a start po wolnym otwarciu urządzenia zaczynałby się od
/// połowy narastania, z trzaskiem.
struct Rampa {
    zrodlo: Box<dyn Source + Send>,
    sterowanie: Arc<Sterowanie>,
    biezaca: f32,
    /// Odwrotność liczby próbek (wszystkich kanałów) na sekundę.
    na_probke: f32,
}

impl Rampa {
    fn nowa(zrodlo: Box<dyn Source + Send>, sterowanie: Arc<Sterowanie>) -> Self {
        let na_sekunde = f64::from(zrodlo.sample_rate().get()) * f64::from(zrodlo.channels().get());
        Rampa { zrodlo, sterowanie, biezaca: 0.0, na_probke: (1.0 / na_sekunde) as f32 }
    }
}

impl Iterator for Rampa {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let cel = self.sterowanie.cel();
        let krok = self.sterowanie.nachylenie() * self.na_probke;
        if self.biezaca < cel {
            self.biezaca = (self.biezaca + krok).min(cel);
        } else if self.biezaca > cel {
            self.biezaca = (self.biezaca - krok).max(cel);
        }
        self.zrodlo.next().map(|x| x * self.biezaca)
    }
}

impl Source for Rampa {
    fn current_span_len(&self) -> Option<usize> {
        self.zrodlo.current_span_len()
    }

    fn channels(&self) -> rodio::ChannelCount {
        self.zrodlo.channels()
    }

    fn sample_rate(&self) -> rodio::SampleRate {
        self.zrodlo.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        self.zrodlo.total_duration()
    }

    fn try_seek(&mut self, pozycja: Duration) -> Result<(), rodio::source::SeekError> {
        self.zrodlo.try_seek(pozycja)
    }
}

struct Otwarte {
    // Strumień i odtwarzacz muszą żyć tak długo, jak gra dźwięk.
    _strumien: MixerDeviceSink,
    _odtwarzacz: Player,
}

/// Urządzenie z `rodio`: domyślne wyjście systemu, otwierane przy każdym starcie dźwięku,
/// więc po wpięciu słuchawek następna Praca gra już w nich.
pub struct Rodio {
    katalog: Option<PathBuf>,
    otwarte: Option<Otwarte>,
    utracone: Arc<AtomicBool>,
    powiadom: Arc<dyn Fn() + Send + Sync>,
    sterowanie: Arc<Sterowanie>,
    /// Wbudowany dźwięk rozkodowany raz: kolejne otwarcia urządzenia nie dekodują go znowu.
    wbudowany: Option<SamplesBuffer>,
}

impl Rodio {
    /// `powiadom` budzi wątek, gdy system zgłosi błąd strumienia (np. wypięte słuchawki).
    pub fn nowe(katalog: Option<PathBuf>, powiadom: impl Fn() + Send + Sync + 'static) -> Self {
        Rodio {
            katalog,
            otwarte: None,
            utracone: Arc::new(AtomicBool::new(false)),
            powiadom: Arc::new(powiadom),
            sterowanie: Arc::new(Sterowanie::default()),
            wbudowany: None,
        }
    }
}

/// Błąd strumienia, po którym trzeba otworzyć urządzenie od nowa. Chwilowe zakłócenia
/// (`Xrun`: niedobór danych w buforze), zmiana trasy dźwięku, którą system załatwia sam
/// (`DeviceChanged`), i odmowa priorytetu czasu rzeczywistego niczego nie psują.
fn wymaga_ponownego_otwarcia(rodzaj: CpalErrorKind) -> bool {
    !matches!(rodzaj, CpalErrorKind::Xrun | CpalErrorKind::DeviceChanged | CpalErrorKind::RealtimeDenied)
}

/// Liczba klatek bufora urządzenia na `BUFOR_URZADZENIA` przy częstotliwości domyślnej konfiguracji.
fn klatki_bufora() -> Option<u32> {
    let urzadzenie = rodio::cpal::default_host().default_output_device()?;
    let czestotliwosc = urzadzenie.default_output_config().ok()?.sample_rate();
    // Parzysta liczba klatek, jak w domyślnych ustawieniach rodio.
    Some((czestotliwosc as f32 * BUFOR_URZADZENIA.as_secs_f32()) as u32 / 2 * 2)
}

fn otworz_strumien(
    utracone: &Arc<AtomicBool>,
    powiadom: &Arc<dyn Fn() + Send + Sync>,
) -> Result<MixerDeviceSink, DeviceSinkError> {
    let zbuduj = |wiekszy_bufor: bool| {
        let mut budowniczy = DeviceSinkBuilder::from_default_device()?;
        if let Some(klatki) = wiekszy_bufor.then(klatki_bufora).flatten() {
            budowniczy = budowniczy.with_buffer_size(BufferSize::Fixed(klatki));
        }
        let (flaga, budzik) = (Arc::clone(utracone), Arc::clone(powiadom));
        budowniczy
            .with_error_callback(move |e: CpalError| {
                if wymaga_ponownego_otwarcia(e.kind()) {
                    log::warn!("błąd strumienia audio: {e}");
                    flaga.store(true, Ordering::SeqCst);
                    budzik();
                } else {
                    log::debug!("strumień audio: {e}");
                }
            })
            .open_sink_or_fallback()
    };
    // Urządzenie, które nie przyjmie większego bufora, dostaje domyślny.
    zbuduj(true).or_else(|_| zbuduj(false))
}

impl Wyjscie for Rodio {
    fn otworz(&mut self) -> Result<(), String> {
        let start = Instant::now();
        self.sterowanie.ustaw(0.0, 0.0);
        let zrodlo = zrodlo_dzwieku(self.katalog.as_deref(), &mut self.wbudowany)?;
        let zrodlo = Rampa::nowa(zrodlo, Arc::clone(&self.sterowanie));
        self.utracone.store(false, Ordering::SeqCst);
        let mut strumien = otworz_strumien(&self.utracone, &self.powiadom).map_err(|e| match e {
            DeviceSinkError::NoDevice => "Nie znalazłam urządzenia do odtwarzania dźwięku.".to_string(),
            inny => format!("Nie mogę otworzyć urządzenia audio: {inny}"),
        })?;
        strumien.log_on_drop(false);
        let odtwarzacz = Player::connect_new(strumien.mixer());
        odtwarzacz.append(zrodlo);
        self.otwarte = Some(Otwarte { _strumien: strumien, _odtwarzacz: odtwarzacz });
        log::info!("dźwięk skupienia: urządzenie audio otwarte w {} ms", start.elapsed().as_millis());
        Ok(())
    }

    fn zamknij(&mut self) {
        self.otwarte = None;
    }

    fn otwarte(&self) -> bool {
        self.otwarte.is_some()
    }

    fn utracone(&self) -> bool {
        self.utracone.load(Ordering::SeqCst)
    }

    fn ustaw(&mut self, cel: f32, nachylenie: f32) {
        self.sterowanie.ustaw(cel, nachylenie);
    }
}

// ---- wątek ----------------------------------------------------------------------------------

enum Wiadomosc {
    Cel(Option<f32>),
    Odsluch(f32, Sender<Result<(), String>>),
    /// System zgłosił błąd strumienia: wątek sprawdza urządzenie.
    Sprawdz,
    Koniec(Sender<()>),
}

/// Uchwyt wątku dźwięku. Wszystkie metody tylko wysyłają polecenie, więc nie blokują
/// (poza `odsluchaj` i `zatrzymaj`, które czekają krótko na odpowiedź).
pub struct Silnik {
    nadawca: Mutex<Sender<Wiadomosc>>,
    watek: Mutex<Option<JoinHandle<()>>>,
}

impl Silnik {
    /// Uruchamia wątek. Urządzenie otwiera się dopiero przy pierwszym dźwięku.
    /// `dostepnosc` dostaje `false`, gdy nie da się otworzyć urządzenia, i `true`, gdy wróci.
    pub fn uruchom(katalog: Option<PathBuf>, dostepnosc: impl Fn(bool) + Send + 'static) -> std::io::Result<Silnik> {
        let (nadawca, odbiorca) = mpsc::channel::<Wiadomosc>();
        let do_budzenia = Mutex::new(nadawca.clone());
        let watek = thread::Builder::new().name("dzwiek".into()).spawn(move || {
            let budzik = move || {
                let _ = do_budzenia.lock().unwrap_or_else(|e| e.into_inner()).send(Wiadomosc::Sprawdz);
            };
            let mut maszyna = Maszyna::nowa(Rodio::nowe(katalog, budzik));
            let mut ostatnio = Instant::now();
            let mut czekanie: Option<Duration> = None;
            loop {
                let wiadomosc = match czekanie {
                    Some(czas) => odbiorca.recv_timeout(czas),
                    None => odbiorca.recv().map_err(|_| RecvTimeoutError::Disconnected),
                };
                // Po długim spaniu pierwszy krok liczy się od teraz, nie od ostatniego obrotu.
                let teraz = Instant::now();
                let dt = if czekanie.is_none() { Duration::ZERO } else { teraz - ostatnio };
                let wynik = match wiadomosc {
                    Ok(Wiadomosc::Cel(cel)) => maszyna.obsluz(dt, |m| m.cel(cel)),
                    Ok(Wiadomosc::Odsluch(poziom, odpowiedz)) => {
                        let mut odsluch = Ok(());
                        let wynik = maszyna.obsluz(dt, |m| odsluch = m.odsluch(poziom));
                        let _ = odpowiedz.send(odsluch);
                        wynik
                    }
                    // Termin albo błąd strumienia: krok sprawdza też, czy urządzenie nie zniknęło.
                    Ok(Wiadomosc::Sprawdz) | Err(RecvTimeoutError::Timeout) => maszyna.krok(dt),
                    Ok(Wiadomosc::Koniec(odpowiedz)) => {
                        maszyna.zatrzymaj(thread::sleep);
                        let _ = odpowiedz.send(());
                        return;
                    }
                    Err(RecvTimeoutError::Disconnected) => {
                        maszyna.zatrzymaj(thread::sleep);
                        return;
                    }
                };
                // Czas spędzony w obsłudze (np. w otwieraniu urządzenia) nie jest czasem narastania.
                ostatnio = Instant::now();
                if let Some(dostepne) = wynik.dostepnosc {
                    dostepnosc(dostepne);
                }
                czekanie = wynik.nastepny;
            }
        })?;
        Ok(Silnik { nadawca: Mutex::new(nadawca), watek: Mutex::new(Some(watek)) })
    }

    fn wyslij(&self, wiadomosc: Wiadomosc) -> bool {
        self.nadawca.lock().unwrap_or_else(|e| e.into_inner()).send(wiadomosc).is_ok()
    }

    /// Poziom, do którego ma dojść głośność (0 do 1), albo `None`, gdy ma zapaść cisza.
    pub fn cel(&self, cel: Option<f32>) {
        self.wyslij(Wiadomosc::Cel(cel));
    }

    /// „Odsłuchaj”: 5 s na podanym poziomie. Zwraca komunikat, gdy nie da się zagrać.
    pub fn odsluchaj(&self, poziom: f32) -> Result<(), String> {
        let (odpowiedz, czeka) = mpsc::channel();
        if !self.wyslij(Wiadomosc::Odsluch(poziom, odpowiedz)) {
            return Err("Dźwięk jest wyłączony.".into());
        }
        czeka.recv_timeout(CZEKANIE_NA_WATEK).unwrap_or_else(|_| Err("Urządzenie audio nie odpowiada.".into()))
    }

    /// Przy zamykaniu programu: szybkie wyciszenie i zamknięcie urządzenia, bez trzasku.
    pub fn zatrzymaj(&self) {
        let (odpowiedz, czeka) = mpsc::channel();
        if self.wyslij(Wiadomosc::Koniec(odpowiedz)) {
            let _ = czeka.recv_timeout(CZEKANIE_NA_WATEK);
        }
        if let Some(watek) = self.watek.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = watek.join();
        }
    }
}

#[cfg(test)]
mod testy {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    const S: Duration = Duration::from_secs(1);

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    /// Przesuwa sterownik o `czas` w krokach po 20 ms i zwraca głośność.
    fn minelo(s: &mut Sterownik, czas: Duration) -> f32 {
        let mut g = s.glosnosc();
        let mut reszta = czas;
        while !reszta.is_zero() {
            let dt = reszta.min(KROK);
            g = s.krok(dt);
            reszta -= dt;
        }
        g
    }

    fn blisko(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.002
    }

    #[test]
    fn glosnosc_narasta_trzy_sekundy_od_ciszy_do_poziomu() {
        let mut s = Sterownik::default();
        s.ustaw_cel(Some(0.25));
        assert!(blisko(minelo(&mut s, ms(1500)), 0.125), "w połowie czasu połowa poziomu");
        assert!(blisko(minelo(&mut s, ms(1480)), 0.25 * 2980.0 / 3000.0));
        assert!(s.trwa_zmiana());
        assert_eq!(minelo(&mut s, ms(40)), 0.25);
        assert!(!s.trwa_zmiana());
    }

    #[test]
    fn narastanie_trwa_tyle_samo_przy_kazdym_poziomie() {
        for poziom in [0.05, 0.25, 1.0] {
            let mut s = Sterownik::default();
            s.ustaw_cel(Some(poziom));
            assert!(s.trwa_zmiana(), "poziom {poziom}");
            minelo(&mut s, NARASTANIE - ms(20));
            assert!(s.trwa_zmiana(), "poziom {poziom}: 20 ms przed końcem");
            minelo(&mut s, ms(20));
            assert!(!s.trwa_zmiana(), "poziom {poziom}: po 3 s");
            assert!(blisko(s.glosnosc(), poziom));
        }
    }

    #[test]
    fn glosnosc_cichnie_poltorej_sekundy_do_ciszy() {
        let mut s = Sterownik::default();
        s.ustaw_cel(Some(0.4));
        minelo(&mut s, 4 * S);
        assert!(blisko(s.glosnosc(), 0.4));
        s.ustaw_cel(None);
        assert!(blisko(minelo(&mut s, ms(750)), 0.2), "w połowie cichnięcia połowa poziomu");
        assert!(!s.cichy());
        minelo(&mut s, ms(740));
        assert!(!s.cichy(), "10 ms przed końcem jeszcze słychać");
        minelo(&mut s, ms(20));
        assert_eq!(s.glosnosc(), 0.0);
        assert!(s.cichy());
    }

    #[test]
    fn pauza_w_trakcie_narastania_cichnie_od_biezacej_glosnosci() {
        let mut s = Sterownik::default();
        s.ustaw_cel(Some(0.5));
        minelo(&mut s, S);
        assert!(blisko(s.glosnosc(), 0.5 / 3.0));
        s.ustaw_cel(None);
        assert!(blisko(minelo(&mut s, ms(750)), 0.5 / 6.0), "cichnie liniowo przez 1,5 s od bieżącego poziomu");
        assert!(!s.cichy());
        minelo(&mut s, ms(760));
        assert!(s.cichy());
    }

    #[test]
    fn wznowienie_w_trakcie_cichniecia_narasta_od_tego_miejsca_bez_skoku() {
        let mut s = Sterownik::default();
        s.ustaw_cel(Some(0.4));
        minelo(&mut s, 4 * S);
        s.ustaw_cel(None);
        let przed = minelo(&mut s, ms(600));
        s.ustaw_cel(Some(0.4));
        let po = s.krok(KROK);
        assert!(po > przed, "od razu rośnie, nie spada do zera ani nie skacze do poziomu");
        assert!(po - przed < 0.01, "krok nie jest skokiem: {przed} -> {po}");
    }

    #[test]
    fn zmiana_suwaka_w_trakcie_gry_przesuwa_glosnosc_plynnie() {
        let mut s = Sterownik::default();
        s.ustaw_cel(Some(0.25));
        minelo(&mut s, 4 * S);
        s.ustaw_cel(Some(0.6));
        let mut poprzednia = s.glosnosc();
        let mut maks_skok: f32 = 0.0;
        for _ in 0..200 {
            let g = s.krok(KROK);
            maks_skok = maks_skok.max((g - poprzednia).abs());
            poprzednia = g;
            if !s.trwa_zmiana() {
                break;
            }
        }
        assert!(blisko(s.glosnosc(), 0.6));
        assert!(maks_skok < 0.01, "największy krok {maks_skok}");
    }

    #[test]
    fn odsluch_gra_piec_sekund_i_wraca_do_celu() {
        let mut s = Sterownik::default();
        s.odsluchaj(0.5);
        assert!(blisko(minelo(&mut s, ms(200)), 0.25), "odsłuch narasta w 0,4 s");
        assert!(blisko(minelo(&mut s, ms(200)), 0.5));
        assert!(blisko(minelo(&mut s, ms(4560)), 0.5), "po 4,96 s nadal gra");
        assert!(s.chce_grac());
        assert_eq!(s.do_konca_odsluchu(), Some(ms(40)));
        minelo(&mut s, ms(60));
        assert_eq!(s.do_konca_odsluchu(), None);
        assert!(!s.chce_grac(), "bez pracy po odsłuchu zapada cisza");
        assert!(s.trwa_zmiana(), "i cichnie, a nie urywa się");
        minelo(&mut s, CICHNIECIE);
        assert!(s.cichy());
    }

    #[test]
    fn jeden_dlugi_krok_przez_koniec_odsluchu_zaczyna_cichniecie_dopiero_od_jego_konca() {
        let mut s = Sterownik::default();
        s.odsluchaj(0.5);
        minelo(&mut s, ms(500));
        assert!(blisko(s.glosnosc(), 0.5));
        // do końca odsłuchu zostaje 4,5 s; krok 4,6 s zostawia na cichnięcie tylko 100 ms
        let g = s.krok(ms(4600));
        assert!(blisko(g, 0.5 - 0.5 / 1.5 * 0.1), "{g}");
        assert_eq!(s.do_konca_odsluchu(), None);
    }

    #[test]
    fn odsluch_w_trakcie_pracy_wraca_do_poziomu_pracy() {
        let mut s = Sterownik::default();
        s.ustaw_cel(Some(0.25));
        minelo(&mut s, 4 * S);
        s.odsluchaj(0.8);
        minelo(&mut s, 2 * S);
        assert!(blisko(s.glosnosc(), 0.8));
        minelo(&mut s, 3 * S + 2 * S);
        assert!(blisko(s.glosnosc(), 0.25), "po odsłuchu wraca do głośności z pracy");
    }

    #[test]
    fn szybkie_wyciszenie_przy_wyjsciu_trwa_120_ms() {
        let mut s = Sterownik::default();
        s.ustaw_cel(Some(0.25));
        minelo(&mut s, 4 * S);
        s.odsluchaj(0.5);
        s.wycisz_szybko();
        minelo(&mut s, ms(100));
        assert!(!s.cichy());
        minelo(&mut s, ms(20));
        assert!(s.cichy());
    }

    #[test]
    fn poziom_poza_zakresem_jest_przycinany() {
        let mut s = Sterownik::default();
        s.ustaw_cel(Some(7.0));
        minelo(&mut s, 4 * S);
        assert_eq!(s.glosnosc(), 1.0);
        s.ustaw_cel(Some(-1.0));
        assert!(!s.chce_grac());
    }

    // ---- maszyna z atrapą urządzenia ----

    #[derive(Default)]
    struct Zapis {
        otwarcia: u32,
        zamkniecia: u32,
        /// Cel i nachylenie ostatnio przekazane urządzeniu.
        cel: f32,
        nachylenie: f32,
        /// Cel ustawiony tuż przed zamknięciem urządzenia.
        cel_przy_zamknieciu: Option<f32>,
        otwarte: bool,
        utracone: bool,
        bledy: u32,
    }

    #[derive(Clone, Default)]
    struct Atrapa(Rc<RefCell<Zapis>>);

    impl Wyjscie for Atrapa {
        fn otworz(&mut self) -> Result<(), String> {
            let mut z = self.0.borrow_mut();
            if z.bledy > 0 {
                z.bledy -= 1;
                return Err("brak urządzenia".into());
            }
            z.otwarcia += 1;
            z.otwarte = true;
            z.utracone = false;
            Ok(())
        }
        fn zamknij(&mut self) {
            let mut z = self.0.borrow_mut();
            if z.otwarte {
                z.zamkniecia += 1;
                z.cel_przy_zamknieciu = Some(z.cel);
            }
            z.otwarte = false;
            z.cel = 0.0;
        }
        fn otwarte(&self) -> bool {
            self.0.borrow().otwarte
        }
        fn utracone(&self) -> bool {
            self.0.borrow().utracone
        }
        fn ustaw(&mut self, cel: f32, nachylenie: f32) {
            let mut z = self.0.borrow_mut();
            z.cel = cel;
            z.nachylenie = nachylenie;
        }
    }

    /// Krok maszyny, jakby wątek czekał dokładnie tyle, ile mu kazała.
    fn przejdz(m: &mut Maszyna<Atrapa>, czas: Duration) -> Vec<bool> {
        let mut zgloszenia = Vec::new();
        let mut reszta = czas;
        while !reszta.is_zero() {
            let dt = reszta.min(KROK);
            if let Some(d) = m.krok(dt).dostepnosc {
                zgloszenia.push(d);
            }
            reszta -= dt;
        }
        zgloszenia
    }

    fn maszyna() -> (Maszyna<Atrapa>, Atrapa) {
        let atrapa = Atrapa::default();
        (Maszyna::nowa(atrapa.clone()), atrapa)
    }

    /// Wątek tak, jak go budzi `Silnik`: śpi dokładnie tyle, ile kazał ostatni `krok`
    /// (albo do polecenia, gdy nic nie czeka na czas), i dopiero wtedy woła `krok`.
    struct Watek {
        m: Maszyna<Atrapa>,
        a: Atrapa,
        nastepny: Option<Duration>,
        od_obrotu: Duration,
    }

    impl Watek {
        fn nowy() -> Self {
            let (m, a) = maszyna();
            Watek { m, a, nastepny: None, od_obrotu: Duration::ZERO }
        }

        /// Polecenie przychodzi w trakcie snu; po śnie bez terminu wątek liczy czas od zera.
        fn polecenie(&mut self, f: impl FnOnce(&mut Maszyna<Atrapa>)) {
            let dt = if self.nastepny.is_some() { self.od_obrotu } else { Duration::ZERO };
            let wynik = self.m.obsluz(dt, f);
            self.od_obrotu = Duration::ZERO;
            self.nastepny = wynik.nastepny;
        }

        /// Mija `czas`, a wątek budzi się tylko wtedy, gdy wypada jego termin.
        fn mija(&mut self, czas: Duration) {
            let mut reszta = czas;
            while let Some(termin) = self.nastepny {
                let do_terminu = termin.saturating_sub(self.od_obrotu);
                if do_terminu > reszta {
                    break;
                }
                reszta -= do_terminu;
                let wynik = self.m.krok(termin);
                self.od_obrotu = Duration::ZERO;
                self.nastepny = wynik.nastepny;
            }
            self.od_obrotu += reszta;
        }

        fn glosnosc(&self) -> f32 {
            self.m.glosnosc()
        }
    }

    #[test]
    fn bez_celu_urzadzenie_nie_jest_otwierane() {
        let (mut m, a) = maszyna();
        przejdz(&mut m, 10 * S);
        assert_eq!(a.0.borrow().otwarcia, 0);
        assert_eq!(m.krok(KROK).nastepny, None, "nic nie czeka na czas, wątek śpi do polecenia");
    }

    #[test]
    fn start_pracy_otwiera_urzadzenie_i_zglasza_dostepnosc_raz() {
        let (mut m, a) = maszyna();
        m.cel(Some(0.25));
        assert_eq!(przejdz(&mut m, ms(100)), vec![true]);
        assert_eq!(a.0.borrow().otwarcia, 1);
        assert!(m.glosnosc() > 0.0);
        assert!(przejdz(&mut m, 5 * S).is_empty(), "dostępność zgłaszana tylko przy zmianie");
        assert!(blisko(m.glosnosc(), 0.25));
    }

    #[test]
    fn narastanie_zaczyna_sie_od_ciszy_takze_gdy_urzadzenie_otwiera_sie_pozniej() {
        let (mut m, a) = maszyna();
        a.0.borrow_mut().bledy = 1;
        m.cel(Some(0.25));
        assert_eq!(przejdz(&mut m, ms(100)), vec![false], "brak urządzenia: zgłoszone");
        assert_eq!(a.0.borrow().otwarcia, 0);
        przejdz(&mut m, 5 * S);
        assert_eq!(a.0.borrow().otwarcia, 1, "ponowienie po 5 s");
        assert!(m.glosnosc() < 0.02, "po otwarciu gra od ciszy, a nie od razu pełnym poziomem");
        przejdz(&mut m, 4 * S);
        assert!(blisko(m.glosnosc(), 0.25));
    }

    #[test]
    fn powtarzany_cel_nie_przyspiesza_ponawiania_otwarcia() {
        let (mut m, a) = maszyna();
        a.0.borrow_mut().bledy = 100;
        m.cel(Some(0.25));
        przejdz(&mut m, ms(100));
        let proby_po_starcie = 100 - a.0.borrow().bledy;
        // rdzeń wysyła ten sam cel przy każdej akcji, tu co 200 ms przez 4 s
        for _ in 0..20 {
            m.cel(Some(0.25));
            przejdz(&mut m, ms(200));
        }
        assert_eq!(100 - a.0.borrow().bledy, proby_po_starcie, "w ciągu 5 s od porażki nie ma nowej próby");
        przejdz(&mut m, 2 * S);
        assert_eq!(100 - a.0.borrow().bledy, proby_po_starcie + 1, "po 5 s jest dokładnie jedna");
    }

    #[test]
    fn odsluch_konczy_sie_po_5_sekundach_takze_gdy_watek_spi_miedzy_terminami() {
        // To odtwarza błąd z nagrania na prawdziwym wyjściu: odsłuch trwał bez końca, bo
        // długie spanie wątku skracało zegar terminów do 250 ms.
        let mut w = Watek::nowy();
        w.polecenie(|m| m.cel(Some(0.25)));
        w.mija(4 * S);
        assert!(blisko(w.glosnosc(), 0.25));
        w.polecenie(|m| m.cel(None));
        w.mija(2 * S);
        assert_eq!(w.glosnosc(), 0.0);
        w.polecenie(|m| m.odsluch(0.5).unwrap());
        w.mija(4 * S + ms(500));
        assert!(blisko(w.glosnosc(), 0.5), "odsłuch jeszcze gra: {}", w.glosnosc());
        w.mija(ms(500) + S);
        assert!(blisko(w.glosnosc(), 0.5 - 0.5 / 1.5), "po 5 s cichnie od poziomu odsłuchu: {}", w.glosnosc());
        w.mija(ms(600));
        assert_eq!(w.glosnosc(), 0.0, "po 6,5 s od początku odsłuchu cisza");
    }

    #[test]
    fn po_odsluchu_w_trakcie_pracy_wraca_poziom_z_pracy_a_wznowienie_dziala_po_dlugim_snie() {
        let mut w = Watek::nowy();
        w.polecenie(|m| m.cel(Some(0.25)));
        w.mija(5 * S);
        w.polecenie(|m| m.odsluch(0.8).unwrap());
        w.mija(4 * S);
        assert!(blisko(w.glosnosc(), 0.8));
        w.mija(3 * S);
        assert!(blisko(w.glosnosc(), 0.25), "wraca do pracy: {}", w.glosnosc());
        w.polecenie(|m| m.cel(None));
        w.mija(10 * S);
        assert_eq!(w.glosnosc(), 0.0);
        w.polecenie(|m| m.cel(Some(0.25)));
        w.mija(S);
        assert!(
            blisko(w.glosnosc(), 0.25 / 3.0),
            "narasta od ciszy mimo wcześniejszego długiego snu: {}",
            w.glosnosc()
        );
    }

    #[test]
    fn wątek_zamyka_urzadzenie_po_30_sekundach_ciszy_mimo_dlugich_snow() {
        let mut w = Watek::nowy();
        w.polecenie(|m| m.cel(Some(0.25)));
        w.mija(5 * S);
        w.polecenie(|m| m.cel(None));
        w.mija(29 * S);
        assert!(w.a.0.borrow().otwarte);
        w.mija(3 * S);
        assert!(!w.a.0.borrow().otwarte);
        assert_eq!(w.nastepny, None, "po zamknięciu nic nie budzi wątku do następnego polecenia");
    }

    #[test]
    fn dostepnosc_wraca_po_udanym_otwarciu() {
        let (mut m, a) = maszyna();
        a.0.borrow_mut().bledy = 2;
        m.cel(Some(0.25));
        let zgloszenia = przejdz(&mut m, 12 * S);
        assert_eq!(zgloszenia, vec![false, true]);
    }

    #[test]
    fn cisza_zamyka_urzadzenie_dopiero_po_30_sekundach() {
        let (mut m, a) = maszyna();
        m.cel(Some(0.25));
        przejdz(&mut m, 5 * S);
        m.cel(None);
        przejdz(&mut m, 2 * S);
        assert_eq!(m.glosnosc(), 0.0);
        assert!(a.0.borrow().otwarte, "po Pauzie urządzenie czeka na Wznów");
        m.cel(Some(0.25));
        przejdz(&mut m, 5 * S);
        assert_eq!(a.0.borrow().otwarcia, 1, "Wznów nie otwiera urządzenia drugi raz");
        m.cel(None);
        przejdz(&mut m, 20 * S);
        assert!(a.0.borrow().otwarte);
        przejdz(&mut m, 12 * S);
        assert!(!a.0.borrow().otwarte, "po 30 s ciszy urządzenie jest zwolnione");
        assert_eq!(a.0.borrow().zamkniecia, 1);
    }

    #[test]
    fn kolejny_start_po_zamknieciu_otwiera_urzadzenie_od_nowa() {
        let (mut m, a) = maszyna();
        m.cel(Some(0.25));
        przejdz(&mut m, 3 * S);
        m.cel(None);
        przejdz(&mut m, 40 * S);
        assert!(!a.0.borrow().otwarte);
        m.cel(Some(0.25));
        przejdz(&mut m, ms(100));
        assert_eq!(a.0.borrow().otwarcia, 2);
    }

    #[test]
    fn wypiete_sluchawki_otwieraja_urzadzenie_od_nowa_z_narastaniem() {
        let (mut m, a) = maszyna();
        m.cel(Some(0.25));
        przejdz(&mut m, 6 * S);
        assert!(blisko(m.glosnosc(), 0.25));
        a.0.borrow_mut().utracone = true;
        przejdz(&mut m, ms(40));
        assert_eq!(a.0.borrow().otwarcia, 2, "otwarte ponownie na nowym urządzeniu");
        assert!(m.glosnosc() < 0.02, "i znów od ciszy");
        przejdz(&mut m, 4 * S);
        assert!(blisko(m.glosnosc(), 0.25));
    }

    #[test]
    fn wypiete_urzadzenie_bez_zastepczego_zglasza_brak_i_ponawia() {
        let (mut m, a) = maszyna();
        m.cel(Some(0.25));
        przejdz(&mut m, 6 * S);
        {
            let mut z = a.0.borrow_mut();
            z.utracone = true;
            z.bledy = 1;
        }
        let zgloszenia = przejdz(&mut m, 12 * S);
        assert_eq!(zgloszenia, vec![false, true]);
        assert_eq!(a.0.borrow().otwarcia, 2);
    }

    #[test]
    fn odsluch_otwiera_urzadzenie_gra_piec_sekund_i_cichnie() {
        let (mut m, a) = maszyna();
        assert_eq!(m.odsluch(0.5), Ok(()));
        assert!(a.0.borrow().otwarte);
        przejdz(&mut m, 2 * S);
        assert!(blisko(m.glosnosc(), 0.5));
        przejdz(&mut m, 3 * S + CICHNIECIE);
        assert_eq!(m.glosnosc(), 0.0);
    }

    #[test]
    fn odsluch_bez_urzadzenia_zwraca_komunikat_i_zglasza_brak() {
        let (mut m, a) = maszyna();
        a.0.borrow_mut().bledy = 1;
        assert_eq!(m.odsluch(0.5), Err("brak urządzenia".to_string()));
        assert_eq!(m.krok(KROK).dostepnosc, Some(false));
        assert!(!a.0.borrow().otwarte);
    }

    #[test]
    fn odsluch_przy_glosnosci_zero_tlumaczy_dlaczego_cicho() {
        let (mut m, a) = maszyna();
        assert!(m.odsluch(0.0).unwrap_err().contains("0"));
        assert_eq!(a.0.borrow().otwarcia, 0);
    }

    #[test]
    fn nastepny_termin_to_krok_przy_zmianie_i_czas_do_konca_odsluchu_przy_stalej_glosnosci() {
        let (mut m, _) = maszyna();
        m.cel(Some(0.25));
        assert_eq!(m.krok(KROK).nastepny, Some(KROK), "narasta");
        przejdz(&mut m, 4 * S);
        let w = m.krok(KROK);
        assert_eq!(
            w.nastepny.map(|d| d <= SPRAWDZANIE_URZADZENIA),
            Some(true),
            "stała głośność: tylko kontrola urządzenia"
        );
        m.odsluch(0.25).unwrap();
        przejdz(&mut m, 4 * S);
        let do_konca = m.krok(KROK).nastepny.unwrap();
        assert!(do_konca <= S, "odsłuch zaraz się skończy: {do_konca:?}");
    }

    #[test]
    fn zatrzymanie_wycisza_w_120_ms_czeka_na_bufor_urzadzenia_i_dopiero_zamyka_je() {
        let (mut m, a) = maszyna();
        m.cel(Some(0.25));
        przejdz(&mut m, 6 * S);
        let mut czekano = Duration::ZERO;
        m.zatrzymaj(|d| czekano += d);
        assert!(!a.0.borrow().otwarte);
        assert_eq!(a.0.borrow().cel_przy_zamknieciu, Some(0.0), "zamknięte dopiero przy ciszy, bez urwania fali");
        assert!(czekano >= OPROZNIENIE_BUFORA + ms(100), "cisza w ok. 120 ms i opróżnienie bufora: {czekano:?}");
        assert!(czekano <= OPROZNIENIE_BUFORA + ms(160), "zamykanie programu nie wlecze się: {czekano:?}");
    }

    #[test]
    fn zatrzymanie_bez_otwartego_urzadzenia_nie_czeka() {
        let (mut m, a) = maszyna();
        let mut czekano = Duration::ZERO;
        m.zatrzymaj(|d| czekano += d);
        assert_eq!(czekano, Duration::ZERO);
        assert_eq!(a.0.borrow().zamkniecia, 0);
    }

    #[test]
    fn dlugi_sen_wątku_nie_przeskakuje_narastania() {
        let (mut m, _) = maszyna();
        m.cel(Some(0.25));
        m.krok(Duration::from_secs(3600));
        assert!(blisko(m.glosnosc(), 0.25 * 0.25 / 3.0), "głośność zmienia się najwyżej o krok 250 ms");
    }

    // ---- plik dźwiękowy ----

    fn probki() -> (u16, u32, Vec<f32>) {
        let b = wbudowany().unwrap();
        (b.channels().get(), b.sample_rate().get(), b.collect())
    }

    /// Amplituda składowej o częstotliwości `f` (algorytm Goertzela, w f64: przy 480 tys. próbek
    /// f32 zbiera za duży błąd).
    fn amplituda(dane: &[f32], f: f64, fs: f64) -> f64 {
        let w = 2.0 * std::f64::consts::PI * f / fs;
        let wsp = 2.0 * w.cos();
        let (mut s1, mut s2) = (0.0f64, 0.0f64);
        for &x in dane {
            let s = f64::from(x) + wsp * s1 - s2;
            s2 = s1;
            s1 = s;
        }
        let moc = s1 * s1 + s2 * s2 - wsp * s1 * s2;
        2.0 * moc.max(0.0).sqrt() / dane.len() as f64
    }

    #[test]
    fn wbudowany_dzwiek_ma_10_s_48_khz_stereo() {
        let (kanaly, czestotliwosc, dane) = probki();
        assert_eq!((kanaly, czestotliwosc), (2, 48_000));
        assert_eq!(dane.len(), 10 * 48_000 * 2);
    }

    #[test]
    fn lewy_kanal_to_200_hz_a_prawy_240_hz_z_szczytem_minus_12_dbfs() {
        let (_, fs, dane) = probki();
        let lewy: Vec<f32> = dane.iter().step_by(2).copied().collect();
        let prawy: Vec<f32> = dane.iter().skip(1).step_by(2).copied().collect();
        let fs = f64::from(fs);
        let szczyt = 10f64.powf(-12.0 / 20.0);
        for (nazwa, kanal, wlasciwa, obca) in [("lewy", &lewy, 200.0, 240.0), ("prawy", &prawy, 240.0, 200.0)] {
            let a_wlasciwa = amplituda(kanal, wlasciwa, fs);
            let a_obca = amplituda(kanal, obca, fs);
            assert!((a_wlasciwa - szczyt).abs() < 0.001, "{nazwa}: amplituda {a_wlasciwa} zamiast {szczyt}");
            assert!(a_obca < 0.001, "{nazwa}: obcy ton {a_obca}");
        }
    }

    #[test]
    fn polaczenie_petli_nie_ma_skoku_wiekszego_niz_zwykla_roznica_sasiednich_probek() {
        let (_, _, dane) = probki();
        let n = dane.len();
        for kanal in 0..2 {
            let x: Vec<f32> = dane.iter().skip(kanal).step_by(2).copied().collect();
            let najwiekszy_w_srodku = x.windows(2).map(|p| (p[1] - p[0]).abs()).fold(0.0f32, f32::max);
            let na_laczeniu = (x[0] - x[x.len() - 1]).abs();
            assert!(
                na_laczeniu <= najwiekszy_w_srodku * 1.05,
                "kanał {kanal}: skok na łączeniu {na_laczeniu} przy największej zwykłej różnicy {najwiekszy_w_srodku}"
            );
        }
        assert_eq!(n % 2, 0);
    }

    #[test]
    fn oba_tony_mieszcza_pelna_liczbe_okresow() {
        // 200 Hz × 10 s = 2000 okresów, 240 Hz × 10 s = 2400: faza na końcu równa fazie na początku.
        let (_, fs, dane) = probki();
        let x0 = (dane[0], dane[1]);
        assert!(x0.0.abs() < 0.01 && x0.1.abs() < 0.01, "start w zerze obu sinusów: {x0:?}");
        let ostatnie = (dane[dane.len() - 2], dane[dane.len() - 1]);
        let krok_l = 2.0 * std::f32::consts::PI * 200.0 / fs as f32;
        let krok_p = 2.0 * std::f32::consts::PI * 240.0 / fs as f32;
        let szczyt = 10f32.powf(-12.0 / 20.0);
        assert!(
            (ostatnie.0 + szczyt * krok_l.sin()).abs() < 0.01,
            "ostatnia próbka lewego tuż przed zerem: {ostatnie:?}"
        );
        assert!(
            (ostatnie.1 + szczyt * krok_p.sin()).abs() < 0.01,
            "ostatnia próbka prawego tuż przed zerem: {ostatnie:?}"
        );
    }

    #[test]
    fn petla_z_repeat_infinite_laczy_sie_bez_skoku_ani_przerwy() {
        // To, co naprawdę trafia do urządzenia: wbudowany dźwięk z `repeat_infinite()`.
        let (_, _, jeden) = probki();
        let okres = jeden.len();
        let trzy: Vec<f32> = wbudowany().unwrap().repeat_infinite().take(3 * okres + 10).collect();
        assert_eq!(trzy.len(), 3 * okres + 10, "pętla nie kończy się ani nie robi przerwy");
        for powtorzenie in 1..3 {
            assert_eq!(&trzy[powtorzenie * okres..(powtorzenie + 1) * okres], &jeden[..], "powtórzenie {powtorzenie}");
        }
        assert_eq!(&trzy[3 * okres..], &jeden[..10]);
        // Różnica sąsiednich próbek na łączeniu nie przekracza zwykłej różnicy w obrębie tonu.
        for kanal in 0..2 {
            let x: Vec<f32> = trzy.iter().skip(kanal).step_by(2).copied().collect();
            let zwykla = x[..x.len() / 3].windows(2).map(|p| (p[1] - p[0]).abs()).fold(0.0f32, f32::max);
            let polowka = okres / 2;
            let na_laczeniach = [polowka - 1, 2 * polowka - 1].map(|i| (x[i + 1] - x[i]).abs());
            assert!(na_laczeniach.iter().all(|r| *r <= zwykla * 1.05), "kanał {kanal}: {na_laczeniach:?} > {zwykla}");
        }
    }

    /// Ręczny test z prawdziwym urządzeniem audio (domyślne wyjście systemu). Gra scenariusz
    /// i wypisuje chwile; nagranie robi się osobno, np. `parec` na monitorze wirtualnego wyjścia:
    /// `cargo test --lib urzadzenie_gra_scenariusz -- --ignored --nocapture`.
    #[test]
    #[ignore = "wymaga urządzenia audio i trwa 20 s; uruchamiany ręcznie"]
    fn urzadzenie_gra_scenariusz() {
        let start = Instant::now();
        let chwila = |opis: &str| println!("[{:5.2} s] {opis}", start.elapsed().as_secs_f32());
        let silnik = Silnik::uruchom(None, |dostepne| println!("dostępność urządzenia: {dostepne}")).unwrap();
        chwila("start pracy: narasta 3 s do 25%");
        silnik.cel(Some(0.25));
        thread::sleep(6 * S);
        chwila("pauza: cichnie 1,5 s");
        silnik.cel(None);
        thread::sleep(3 * S);
        chwila("odsłuch na 50%: 5 s");
        println!("odsłuch: {:?}", silnik.odsluchaj(0.5));
        thread::sleep(7 * S);
        chwila("wznowienie: narasta 3 s");
        silnik.cel(Some(0.25));
        thread::sleep(4 * S);
        chwila("zamknięcie programu: cisza w 120 ms");
        silnik.zatrzymaj();
        chwila("koniec");
    }

    fn jedynki() -> Box<dyn Source + Send> {
        let jedynka = vec![1.0f32; 48_000];
        Box::new(SamplesBuffer::new(1.try_into().unwrap(), 48_000.try_into().unwrap(), jedynka).repeat_infinite())
    }

    #[test]
    fn rampa_zaczyna_od_pierwszej_odtworzonej_probki_a_nie_od_chwili_polecenia() {
        // Wątek zlecił narastanie do 0,25 w 3 s, ale urządzenie zaczyna grać dopiero teraz
        // (po wolnym otwarciu): rampa startuje od zera, bez przeskoku do poziomu z wątku.
        let sterowanie = Arc::new(Sterowanie::default());
        sterowanie.ustaw(0.25, 0.25 / 3.0);
        let mut rampa = Rampa::nowa(jedynki(), sterowanie);
        let pierwsza_sekunda: Vec<f32> = rampa.by_ref().take(48_000).collect();
        assert!(pierwsza_sekunda[0] < 1e-5, "pierwsza próbka jest cicha: {}", pierwsza_sekunda[0]);
        assert!((pierwsza_sekunda[47_999] - 0.25 / 3.0).abs() < 1e-4, "po 1 s jedna trzecia drogi");
        let reszta: Vec<f32> = rampa.by_ref().take(3 * 48_000).collect();
        let wszystko: Vec<f32> = pierwsza_sekunda.into_iter().chain(reszta).collect();
        assert!((wszystko[3 * 48_000 - 1] - 0.25).abs() < 1e-4, "po 3 s cel");
        assert_eq!(*wszystko.last().unwrap(), 0.25);
        let skoki: Vec<f32> = wszystko.windows(2).map(|p| p[1] - p[0]).collect();
        let najwiekszy = skoki.iter().copied().fold(0.0f32, f32::max);
        assert!(najwiekszy <= 0.25 / 3.0 / 48_000.0 * 1.05, "liniowo, bez schodków: {najwiekszy}");
        assert!(skoki.iter().all(|r| *r >= 0.0), "nigdy nie maleje");
    }

    #[test]
    fn rampa_cichnie_liniowo_do_zera_nawet_gdy_urzadzenie_nie_doszlo_jeszcze_do_poziomu_z_watku() {
        let sterowanie = Arc::new(Sterowanie::default());
        sterowanie.ustaw(0.25, 0.25 / 3.0);
        let mut rampa = Rampa::nowa(jedynki(), Arc::clone(&sterowanie));
        rampa.by_ref().take(48_000).for_each(drop); // gra 1 s, jest przy 1/3 poziomu
        sterowanie.ustaw(0.0, 0.25 / 1.5); // pauza: tempo z wątku, który uważa, że jest przy 0,25
        let cisza: Vec<f32> = rampa.by_ref().take(2 * 48_000).collect();
        assert!(cisza.windows(2).all(|p| p[1] <= p[0]), "monotonicznie maleje");
        let pierwsze_zero = cisza.iter().position(|x| *x == 0.0).unwrap();
        assert!(pierwsze_zero as f32 / 48_000.0 < 0.55, "z 1/3 poziomu do zera szybciej niż 1,5 s: {pierwsze_zero}");
        assert_eq!(*cisza.last().unwrap(), 0.0);
        assert!(cisza[..pierwsze_zero].windows(2).all(|p| (p[0] - p[1]) <= 0.25 / 1.5 / 48_000.0 * 1.05));
    }

    #[test]
    fn rampa_przy_zmianie_celu_w_trakcie_zmienia_kierunek_bez_skoku() {
        let sterowanie = Arc::new(Sterowanie::default());
        sterowanie.ustaw(0.5, 0.5 / 3.0);
        let mut rampa = Rampa::nowa(jedynki(), Arc::clone(&sterowanie));
        let mut poprzednia = 0.0;
        let mut najwiekszy_skok: f32 = 0.0;
        for i in 0..4 * 48_000 {
            if i == 48_000 {
                sterowanie.ustaw(0.1, 0.5 / 1.5); // suwak w dół w trakcie narastania
            }
            let x = rampa.next().unwrap();
            najwiekszy_skok = najwiekszy_skok.max((x - poprzednia).abs());
            poprzednia = x;
        }
        assert!(najwiekszy_skok <= 0.5 / 1.5 / 48_000.0 * 1.05, "{najwiekszy_skok}");
        assert_eq!(poprzednia, 0.1);
    }

    #[test]
    fn urzadzenie_dostaje_cel_i_nachylenie_a_rampe_liczy_samo() {
        let (mut m, a) = maszyna();
        m.cel(Some(0.25));
        przejdz(&mut m, 4 * S);
        assert_eq!(a.0.borrow().cel, 0.25);
        assert!((a.0.borrow().nachylenie - 0.25 / 3.0).abs() < 1e-4, "narastanie 3 s: {}", a.0.borrow().nachylenie);
        m.cel(None);
        przejdz(&mut m, ms(100));
        assert_eq!(a.0.borrow().cel, 0.0);
        assert!((a.0.borrow().nachylenie - 0.25 / 1.5).abs() < 1e-4, "cichnięcie 1,5 s: {}", a.0.borrow().nachylenie);
    }

    #[test]
    fn tylko_utrata_urzadzenia_wymaga_ponownego_otwarcia_a_niedobor_danych_w_buforze_nie() {
        assert!(!wymaga_ponownego_otwarcia(CpalErrorKind::Xrun), "chwilowy trzask, strumień gra dalej");
        assert!(!wymaga_ponownego_otwarcia(CpalErrorKind::DeviceChanged), "system sam przełączył trasę");
        assert!(!wymaga_ponownego_otwarcia(CpalErrorKind::RealtimeDenied));
        assert!(wymaga_ponownego_otwarcia(CpalErrorKind::DeviceNotAvailable), "wypięte słuchawki");
        assert!(wymaga_ponownego_otwarcia(CpalErrorKind::StreamInvalidated));
        assert!(wymaga_ponownego_otwarcia(CpalErrorKind::BackendError));
    }

    #[test]
    fn wlasny_plik_to_wav_przed_mp3_a_bez_nich_gra_wbudowany() {
        let katalog = tempfile::tempdir().unwrap();
        assert_eq!(wlasny_plik(katalog.path()), None);
        std::fs::write(katalog.path().join("skupienie.mp3"), b"x").unwrap();
        assert_eq!(wlasny_plik(katalog.path()), Some(katalog.path().join("skupienie.mp3")));
        std::fs::write(katalog.path().join("skupienie.wav"), b"x").unwrap();
        assert_eq!(wlasny_plik(katalog.path()), Some(katalog.path().join("skupienie.wav")));
    }

    #[test]
    fn uszkodzony_wlasny_plik_nie_psuje_dzwieku_wbudowany_gra_zamiast_niego() {
        let katalog = tempfile::tempdir().unwrap();
        std::fs::write(katalog.path().join("skupienie.wav"), b"to nie jest dzwiek").unwrap();
        let mut zrodlo = zrodlo_dzwieku(Some(katalog.path()), &mut None).expect("wbudowany dźwięk jako zastępstwo");
        assert_eq!(zrodlo.channels().get(), 2);
        assert_eq!(zrodlo.sample_rate().get(), 48_000);
        assert!(zrodlo.next().is_some());
    }

    #[test]
    fn wlasny_wav_gra_zamiast_wbudowanego() {
        let katalog = tempfile::tempdir().unwrap();
        // jedna sekunda ciszy mono 8 kHz
        let mut wav = Vec::new();
        let dane = vec![0u8; 8000 * 2];
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + dane.len() as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&8000u32.to_le_bytes());
        wav.extend_from_slice(&16000u32.to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(dane.len() as u32).to_le_bytes());
        wav.extend_from_slice(&dane);
        std::fs::write(katalog.path().join("skupienie.wav"), wav).unwrap();
        let zrodlo = zrodlo_dzwieku(Some(katalog.path()), &mut None).unwrap();
        assert_eq!(zrodlo.channels().get(), 1);
        assert_eq!(zrodlo.sample_rate().get(), 8000);
    }
}
