//! Integracja z systemem (SPEC.md, B5 i B6): blokada i uśpienie, bezczynność,
//! pełny ekran, ukrycie przed nagrywaniem i okno bez kradzieży fokusu.
//! Na Windows przez crate `windows`; na innych systemach zaślepki, żeby dało
//! się pracować nad interfejsem.

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

#[cfg(windows)]
mod win;
#[cfg(windows)]
pub use win::*;

#[cfg(not(windows))]
mod inne;
#[cfg(not(windows))]
pub use inne::*;
