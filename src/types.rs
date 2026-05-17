use std::cell::RefCell;
use std::collections::HashMap;
use indexmap::IndexMap;
use rustc_hash::FxBuildHasher;

thread_local! {
    pub static SYMBOLS: RefCell<SymbolTable> = RefCell::new(SymbolTable::default());
}

#[derive(Default)]
pub struct SymbolTable {
    map: HashMap<String, u32>,
    vec: Vec<String>,
}

impl SymbolTable {
    fn intern(&mut self, name: &str) -> u32 {
        if let Some(&id) = self.map.get(name) {
            id
        } else {
            let id = self.vec.len() as u32;
            self.map.insert(name.to_string(), id);
            self.vec.push(name.to_string());
            id
        }
    }

    fn lookup(&self, id: u32) -> String {
        self.vec
            .get(id as usize)
            .cloned()
            .unwrap_or_else(|| format!("id:{}", id))
    }
}

pub fn intern(name: &str) -> u32 {
    SYMBOLS.with(|s| s.borrow_mut().intern(name))
}

pub fn lookup(id: u32) -> String {
    SYMBOLS.with(|s| s.borrow().lookup(id))
}

#[allow(unused)]
pub fn has_symbol<'a>(id: u32, name: &str) -> bool {
    SYMBOLS.with(|s| s.borrow().vec.get(id as usize).is_some_and(|s| s.as_str() == name))
}

type RecordMap<T> = IndexMap<u32, T, FxBuildHasher>;

#[derive(Debug, Clone)]
pub struct Record<T> {
    fields: RecordMap<T>,
}

impl<T> Record<T> {
    pub fn new() -> Self {
        Self {
            fields: RecordMap::with_hasher(FxBuildHasher::default()),
        }
    }

    pub fn populate(mut self, iter: impl Iterator<Item = (u32, T)>) -> Self {
        self.fields.extend(iter);
        self
    }

    pub fn fields(&self) -> &RecordMap<T> {
        &self.fields
    }

    pub fn fields_mut(&mut self) -> &mut RecordMap<T> {
        &mut self.fields
    }

    pub fn into_fields(self) -> RecordMap<T> {
        self.fields
    }
}
