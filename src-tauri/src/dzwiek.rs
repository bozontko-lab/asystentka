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
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
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
    /// Cel (starsze 32 bity) i nachylenie (młodsze) w jednym słowie: wątek audio czyta
    /// zawsze parę z jednego zapisu, nigdy nowy cel ze starym, np. zerowym, tempem.
    para: AtomicU64,
}

impl Sterowanie {
    fn ustaw(&self, cel: f32, nachylenie: f32) {
        let para = (u64::from(cel.to_bits()) << 32) | u64::from(nachylenie.to_bits());
        self.para.store(para, Ordering::Release);
    }

    /// Cel i nachylenie z jednego zapisu.
    fn para(&self) -> (f32, f32) {
        let para = self.para.load(Ordering::Acquire);
        (f32::from_bits((para >> 32) as u32), f32::from_bits(para as u32))
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
        let (cel, nachylenie) = self.sterowanie.para();
        let krok = nachylenie * self.na_probke;
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
mod testy;
