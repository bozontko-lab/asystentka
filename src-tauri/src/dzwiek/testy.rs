//! Testy modułu `dzwiek`, wydzielone z `dzwiek.rs`, żeby kod i testy dało się czytać osobno.

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
    assert!(blisko(w.glosnosc(), 0.25 / 3.0), "narasta od ciszy mimo wcześniejszego długiego snu: {}", w.glosnosc());
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
    assert!((ostatnie.0 + szczyt * krok_l.sin()).abs() < 0.01, "ostatnia próbka lewego tuż przed zerem: {ostatnie:?}");
    assert!((ostatnie.1 + szczyt * krok_p.sin()).abs() < 0.01, "ostatnia próbka prawego tuż przed zerem: {ostatnie:?}");
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
fn cel_i_nachylenie_czytane_przez_watek_audio_sa_zawsze_z_jednego_zapisu() {
    let sterowanie = Arc::new(Sterowanie::default());
    let koniec = Arc::new(AtomicBool::new(false));
    let pisarz = {
        let (sterowanie, koniec) = (Arc::clone(&sterowanie), Arc::clone(&koniec));
        thread::spawn(move || {
            let mut i = 0u32;
            while !koniec.load(Ordering::SeqCst) {
                let cel = (i % 1000) as f32 + 1.0;
                sterowanie.ustaw(cel, cel * 10.0);
                i = i.wrapping_add(1);
            }
        })
    };
    let mut niezgodne = 0;
    for _ in 0..2_000_000 {
        let (cel, nachylenie) = sterowanie.para();
        if cel != 0.0 && nachylenie != cel * 10.0 {
            niezgodne += 1;
        }
    }
    koniec.store(true, Ordering::SeqCst);
    pisarz.join().unwrap();
    assert_eq!(niezgodne, 0, "nowy cel ze starym nachyleniem");
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
