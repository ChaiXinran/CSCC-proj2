//! Symbol primitive type and global symbol registry.

/// Stable handle into the symbol registry. Each id uniquely identifies one symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymbolId(pub u32);

/// A Symbol value: an optional description string.
#[derive(Debug, Clone)]
pub struct Symbol {
    pub description: Option<String>,
}

/// Well-known symbol ids assigned at registry construction time (ids 0–5).
#[derive(Debug, Clone, Copy)]
pub struct WellKnownSymbols {
    pub to_primitive: SymbolId,
    pub to_string_tag: SymbolId,
    pub iterator: SymbolId,
    pub has_instance: SymbolId,
    pub is_concat_spreadable: SymbolId,
    pub species: SymbolId,
}

/// Global symbol store shared by one NativeContext isolate.
#[derive(Debug)]
pub struct SymbolRegistry {
    symbols: Vec<Symbol>,
    pub well_known: WellKnownSymbols,
}

impl SymbolRegistry {
    pub fn new() -> Self {
        let mut symbols: Vec<Symbol> = Vec::new();

        // Well-known symbols occupy fixed ids 0–5.
        for desc in [
            "Symbol.toPrimitive",
            "Symbol.toStringTag",
            "Symbol.iterator",
            "Symbol.hasInstance",
            "Symbol.isConcatSpreadable",
            "Symbol.species",
        ] {
            symbols.push(Symbol {
                description: Some(desc.into()),
            });
        }

        Self {
            well_known: WellKnownSymbols {
                to_primitive: SymbolId(0),
                to_string_tag: SymbolId(1),
                iterator: SymbolId(2),
                has_instance: SymbolId(3),
                is_concat_spreadable: SymbolId(4),
                species: SymbolId(5),
            },
            symbols,
        }
    }

    /// Allocate a fresh user symbol with an optional description.
    pub fn create(&mut self, description: Option<String>) -> SymbolId {
        let id = SymbolId(self.symbols.len() as u32);
        self.symbols.push(Symbol { description });
        id
    }

    #[must_use]
    pub fn get(&self, id: SymbolId) -> Option<&Symbol> {
        self.symbols.get(id.0 as usize)
    }

    #[must_use]
    pub fn description(&self, id: SymbolId) -> Option<&str> {
        self.get(id)?.description.as_deref()
    }
}

impl Default for SymbolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn well_known_symbols_have_fixed_ids() {
        let reg = SymbolRegistry::new();
        assert_eq!(reg.well_known.to_primitive, SymbolId(0));
        assert_eq!(reg.well_known.to_string_tag, SymbolId(1));
        assert_eq!(reg.well_known.iterator, SymbolId(2));
    }

    #[test]
    fn user_symbols_get_unique_ids() {
        let mut reg = SymbolRegistry::new();
        let a = reg.create(Some("a".into()));
        let b = reg.create(None);
        assert_ne!(a, b);
    }

    #[test]
    fn description_is_accessible() {
        let mut reg = SymbolRegistry::new();
        let id = reg.create(Some("my symbol".into()));
        assert_eq!(reg.description(id), Some("my symbol"));
        let id2 = reg.create(None);
        assert_eq!(reg.description(id2), None);
    }

    #[test]
    fn well_known_descriptions_match_spec() {
        let reg = SymbolRegistry::new();
        assert_eq!(
            reg.description(reg.well_known.to_primitive),
            Some("Symbol.toPrimitive")
        );
        assert_eq!(
            reg.description(reg.well_known.to_string_tag),
            Some("Symbol.toStringTag")
        );
    }
}
