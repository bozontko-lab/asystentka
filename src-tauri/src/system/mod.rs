//! Integracja z systemem (SPEC.md, B5 i B6): blokada i uśpienie, bezczynność,
//! pełny ekran, ukrycie przed nagrywaniem i okno bez kradzieży fokusu.
//! Na Windows przez crate `windows`; na innych systemach zaślepki, żeby dało
//! się pracować nad interfejsem.

use crate::okno::Prostokat;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
pub enum ZdarzenieSystemowe {
    Blokada,
    Odblokowanie,
    Uspienie,
    Wybudzenie,
    /// Wylogowanie albo wyłączanie Windows.
    KoniecSesji,
}

pub type Obsluga = Box<dyn Fn(ZdarzenieSystemowe) + Send + Sync>;

/// Okno na pierwszym planie, jak widzi je system (SPEC.md, B5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
pub struct PierwszyPlan {
    /// Zajmuje cały swój monitor, razem z paskiem zadań. Pulpit i powłoka (pasek zadań) nie wchodzą w grę.
    pub pokrywa_monitor: bool,
    /// Zmaksymalizowane albo z paskiem tytułu: zwykła praca, a nie pełny ekran.
    pub zwykle_okno: bool,
    /// Cały monitor, na którym to okno leży.
    pub monitor: Prostokat,
}

/// Dane do decyzji o chowaniu widgetu przy pełnym ekranie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StanPelnegoEkranu {
    pub pierwszy_plan: Option<PierwszyPlan>,
    /// `SHQueryUserNotificationState`: pełny ekran D3D, tryb prezentacji albo „zajęty”.
    /// Dotyczy całej sesji, nie konkretnego monitora.
    pub sygnal_systemu: bool,
}

/// Widget chowa się tylko wtedy, gdy pełny ekran jest na jego monitorze (B5, B6):
/// okno na pierwszym planie musi pokrywać monitor widgetu. Sygnał systemu
/// zdejmuje tylko wyjątek dla zwykłych okien (zmaksymalizowane okno przy
/// ukrytym pasku zadań), ale nie przesądza o monitorze. Dzięki temu gra na
/// jednym monitorze nie chowa widgetu stojącego na drugim.
pub fn pelny_ekran_na_monitorze(stan: &StanPelnegoEkranu, monitor_widgetu: &Prostokat) -> bool {
    let Some(p) = stan.pierwszy_plan else { return false };
    p.monitor.srodek_w(monitor_widgetu) && p.pokrywa_monitor && (!p.zwykle_okno || stan.sygnal_systemu)
}

/// Proces nagrywania ekranu, przy którym milknie dźwięk skupienia (B7).
pub const PROCES_OBS: &str = "obs64.exe";

/// Nazwy plików wykonywalnych na Windows nie rozróżniają wielkości liter.
#[cfg_attr(not(windows), allow(dead_code))]
fn nazwa_procesu_pasuje(proces: &str, szukany: &str) -> bool {
    proces.eq_ignore_ascii_case(szukany)
}

#[cfg(windows)]
mod win;
#[cfg(windows)]
pub use win::*;

#[cfg(not(windows))]
mod inne;
#[cfg(not(windows))]
pub use inne::*;

#[cfg(test)]
mod testy {
    use super::*;

    const GLOWNY: Prostokat = Prostokat { x: 0, y: 0, szer: 1920, wys: 1080 };
    const DRUGI: Prostokat = Prostokat { x: 1920, y: 0, szer: 2560, wys: 1440 };

    fn pierwszy_plan(monitor: Prostokat, pokrywa: bool, zwykle: bool) -> StanPelnegoEkranu {
        StanPelnegoEkranu {
            pierwszy_plan: Some(PierwszyPlan { pokrywa_monitor: pokrywa, zwykle_okno: zwykle, monitor }),
            sygnal_systemu: false,
        }
    }

    #[test]
    fn gra_na_monitorze_widgetu_chowa_go() {
        assert!(pelny_ekran_na_monitorze(&pierwszy_plan(DRUGI, true, false), &DRUGI));
    }

    #[test]
    fn gra_na_drugim_monitorze_nie_chowa_widgetu() {
        assert!(!pelny_ekran_na_monitorze(&pierwszy_plan(GLOWNY, true, false), &DRUGI));
        assert!(!pelny_ekran_na_monitorze(&pierwszy_plan(DRUGI, true, false), &GLOWNY));
    }

    #[test]
    fn sygnal_systemu_nie_chowa_widgetu_na_innym_monitorze() {
        // gra na głównym, ale ostrość ma zwykłe okno na drugim: system wciąż mówi „pełny ekran”
        let stan = StanPelnegoEkranu { sygnal_systemu: true, ..pierwszy_plan(DRUGI, false, true) };
        assert!(!pelny_ekran_na_monitorze(&stan, &DRUGI));
        let stan = StanPelnegoEkranu { sygnal_systemu: true, ..pierwszy_plan(GLOWNY, true, false) };
        assert!(!pelny_ekran_na_monitorze(&stan, &DRUGI));
    }

    #[test]
    fn zmaksymalizowane_okno_to_praca_chyba_ze_system_mowi_pelny_ekran() {
        let mut stan = pierwszy_plan(DRUGI, true, true);
        assert!(!pelny_ekran_na_monitorze(&stan, &DRUGI));
        stan.sygnal_systemu = true;
        assert!(pelny_ekran_na_monitorze(&stan, &DRUGI));
    }

    #[test]
    fn okno_ktore_nie_pokrywa_monitora_nie_chowa_widgetu() {
        assert!(!pelny_ekran_na_monitorze(&pierwszy_plan(DRUGI, false, false), &DRUGI));
    }

    #[test]
    fn nazwa_procesu_nie_rozroznia_wielkosci_liter_ale_wymaga_calej_nazwy() {
        assert!(nazwa_procesu_pasuje("obs64.exe", PROCES_OBS));
        assert!(nazwa_procesu_pasuje("OBS64.EXE", PROCES_OBS));
        assert!(!nazwa_procesu_pasuje("obs64.exe.bak", PROCES_OBS));
        assert!(!nazwa_procesu_pasuje("notobs64.exe", PROCES_OBS));
        assert!(!nazwa_procesu_pasuje("obs.exe", PROCES_OBS));
    }

    #[test]
    fn brak_okna_na_pierwszym_planie_nie_chowa_widgetu() {
        let stan = StanPelnegoEkranu { pierwszy_plan: None, sygnal_systemu: true };
        assert!(!pelny_ekran_na_monitorze(&stan, &GLOWNY));
    }
}
