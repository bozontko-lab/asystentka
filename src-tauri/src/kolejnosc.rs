//! Kolejność skutków między wątkami (Z-10, Z-11). Bez zależności od Tauri, żeby dało się
//! to sprawdzić testami.
//!
//! `Aplikacja::wykonaj` zwalnia blokadę rdzenia przed rozesłaniem skutków (wywołania okna
//! i menu mogą czekać na wątek główny), więc wolniejszy wątek mógłby rozesłać starszą
//! migawkę po nowszej. [`Najnowsze`] pamięta skutki z numerem wersji nadanym pod blokadą
//! rdzenia i zawsze stosuje najnowsze.

use std::sync::Mutex;
use std::time::{Duration, Instant};

fn zablokuj<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Najnowsze skutki stanu: starsza wersja zgłoszona po nowszej przepada, a stosowanie
/// kończy się zawsze na najnowszej wersji, także gdy nowsza przyjdzie w trakcie.
pub struct Najnowsze<T> {
    stan: Mutex<(u64, Option<T>)>,
}

impl<T: Clone> Najnowsze<T> {
    pub fn new() -> Self {
        Najnowsze { stan: Mutex::new((0, None)) }
    }

    /// Zapisuje skutki wersji `wersja`, chyba że jest już nowsza. Zwraca, czy zapisał.
    pub fn zglos(&self, wersja: u64, skutki: T) -> bool {
        let mut stan = zablokuj(&self.stan);
        if wersja < stan.0 {
            return false;
        }
        *stan = (wersja, Some(skutki));
        true
    }

    /// Stosuje najnowsze skutki poza blokadą (wywołania mogą czekać na wątek główny).
    /// Jeśli w trakcie przyszła nowsza wersja, stosuje także ją, więc ostatnie słowo ma
    /// zawsze najnowszy stan, nawet gdy wątek ze starszym stanem dojdzie tu później.
    pub fn zastosuj(&self, mut zastosuj: impl FnMut(&T)) {
        let mut zastosowana = None;
        loop {
            let (wersja, skutki) = {
                let stan = zablokuj(&self.stan);
                (stan.0, stan.1.clone())
            };
            if zastosowana == Some(wersja) {
                return;
            }
            let Some(skutki) = skutki else { return };
            zastosuj(&skutki);
            zastosowana = Some(wersja);
        }
    }
}

impl<T: Clone> Default for Najnowsze<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Co zrobić z sygnałem zmiany pliku zadań.
#[derive(Debug, PartialEq, Eq)]
pub enum ZmianaPliku {
    Brak,
    /// Minęła chwila spokoju: odśwież zadania (sygnał jest już wyzerowany).
    Odswiez,
    /// Zapisy jeszcze trwają: obudź się o tej chwili.
    Czekaj(Instant),
}

/// Sprawdza i zeruje sygnał zmiany pliku pod jedną blokadą. Świeży sygnał od obserwatora,
/// który przyjdzie między sprawdzeniem a zerowaniem, nie może przepaść (Z-11).
pub fn odbierz_zmiane_pliku(sygnal: &Mutex<Option<Instant>>, teraz: Instant, spokoj: Duration) -> ZmianaPliku {
    let mut sygnal = zablokuj(sygnal);
    match *sygnal {
        None => ZmianaPliku::Brak,
        Some(od) if teraz.saturating_duration_since(od) >= spokoj => {
            *sygnal = None;
            ZmianaPliku::Odswiez
        }
        Some(od) => ZmianaPliku::Czekaj(od + spokoj),
    }
}

#[cfg(test)]
mod testy {
    use super::*;

    #[test]
    fn starsza_wersja_zgloszona_po_nowszej_przepada() {
        let n = Najnowsze::new();
        assert!(n.zglos(2, "praca"));
        assert!(!n.zglos(1, "lista"), "wątek ze starszym stanem przyszedł później");
        let mut zastosowane = Vec::new();
        n.zastosuj(|s| zastosowane.push(*s));
        assert_eq!(zastosowane, vec!["praca"]);
    }

    #[test]
    fn nowsza_wersja_w_trakcie_stosowania_jest_stosowana_na_koniec() {
        let n = Najnowsze::new();
        n.zglos(1, "lista");
        let mut zastosowane = Vec::new();
        n.zastosuj(|s| {
            zastosowane.push(*s);
            // inny wątek zgłasza nowszy stan, gdy ten jeszcze stosuje starszy
            if *s == "lista" {
                n.zglos(2, "praca");
            }
        });
        assert_eq!(zastosowane, vec!["lista", "praca"], "ostatnie słowo ma najnowszy stan");
    }

    #[test]
    fn wolny_watek_ze_starym_stanem_nie_cofa_nowszego() {
        // Wątek A zgłasza wersję 1, ale stosuje dopiero po wątku B z wersją 2.
        let n = Najnowsze::new();
        n.zglos(1, "lista");
        n.zglos(2, "praca");
        let mut ekran = "";
        n.zastosuj(|s| ekran = s); // B
        n.zastosuj(|s| ekran = s); // A, spóźniony
        assert_eq!(ekran, "praca");
    }

    #[test]
    fn bez_zgloszenia_nic_nie_jest_stosowane() {
        let n: Najnowsze<&str> = Najnowsze::new();
        n.zastosuj(|_| panic!("nie ma czego stosować"));
    }

    #[test]
    fn rownolegle_watki_koncza_na_najnowszej_wersji() {
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::sync::Arc;
        let n = Arc::new(Najnowsze::new());
        let ekran = Arc::new(AtomicU64::new(0));
        let watki: Vec<_> = (1..=200u64)
            .map(|wersja| {
                let (n, ekran) = (Arc::clone(&n), Arc::clone(&ekran));
                std::thread::spawn(move || {
                    n.zglos(wersja, wersja);
                    n.zastosuj(|w| ekran.store(*w, Ordering::SeqCst));
                })
            })
            .collect();
        for w in watki {
            w.join().unwrap();
        }
        assert_eq!(ekran.load(Ordering::SeqCst), 200);
    }

    const SPOKOJ: Duration = Duration::from_millis(300);

    #[test]
    fn zmiana_pliku_czeka_na_chwile_spokoju_i_zeruje_sygnal() {
        let start = Instant::now();
        let sygnal = Mutex::new(None);
        assert_eq!(odbierz_zmiane_pliku(&sygnal, start, SPOKOJ), ZmianaPliku::Brak);
        *sygnal.lock().unwrap() = Some(start);
        assert_eq!(
            odbierz_zmiane_pliku(&sygnal, start + Duration::from_millis(100), SPOKOJ),
            ZmianaPliku::Czekaj(start + SPOKOJ)
        );
        assert_eq!(odbierz_zmiane_pliku(&sygnal, start + SPOKOJ, SPOKOJ), ZmianaPliku::Odswiez);
        assert_eq!(*sygnal.lock().unwrap(), None);
    }

    #[test]
    fn swiezy_sygnal_nie_jest_kasowany_razem_ze_starym() {
        // Drugi zapis edytora przychodzi tuż przed sprawdzeniem: zostaje i odświeża później.
        let start = Instant::now();
        let sygnal = Mutex::new(Some(start + Duration::from_millis(350)));
        let teraz = start + Duration::from_millis(400);
        assert_eq!(
            odbierz_zmiane_pliku(&sygnal, teraz, SPOKOJ),
            ZmianaPliku::Czekaj(start + Duration::from_millis(650))
        );
        assert!(sygnal.lock().unwrap().is_some(), "sygnał zostaje do następnego sprawdzenia");
    }
}
