use std::collections::hash_map::{IntoValues, Values, ValuesMut};
use std::collections::HashMap;
use std::ops::Index;

pub trait Named {
    fn name(&self) -> &str;
}

#[derive(Debug, PartialEq)]
pub struct NamedCollection<V: Named + PartialEq>(HashMap<String, V>);

impl<V: Named + PartialEq> NamedCollection<V> {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn insert(&mut self, item: V) {
        let key = item.name().to_string();
        self.0.insert(key, item);
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn contains(&self, v: &V) -> bool {
        // FIXME: Revisit, may not be performant
        self.0.values().any(|item| item == v)
    }
    pub fn contains_name(&self, k: &str) -> bool {
        self.0.contains_key(k)
    }

    pub fn get(&self, k: &str) -> Option<&V> {
        self.0.get(k)
    }

    pub fn get_mut(&mut self, k: &str) -> Option<&mut V> {
        self.0.get_mut(k)
    }

    pub fn names(&self) -> Vec<&String> {
        self.0.keys().collect()
    }
}

impl<V: Named + PartialEq> IntoIterator for NamedCollection<V> {
    type Item = V;
    type IntoIter = IntoValues<String, V>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_values()
    }
}

impl<'a, V: Named + PartialEq> IntoIterator for &'a NamedCollection<V> {
    type Item = &'a V;
    type IntoIter = Values<'a, String, V>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.values()
    }
}

impl<'a, V: Named + PartialEq> IntoIterator for &'a mut NamedCollection<V> {
    type Item = &'a mut V;
    type IntoIter = ValuesMut<'a, String, V>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.values_mut()
    }
}

impl<V: Named + PartialEq> Index<&str> for NamedCollection<V> {
    type Output = V;
    fn index(&self, index: &str) -> &Self::Output {
        &self.0[index]
    }
}

impl<V: Named + PartialEq> Extend<V> for NamedCollection<V> {
    fn extend<T: IntoIterator<Item = V>>(&mut self, iter: T) {
        for item in iter {
            self.insert(item);
        }
    }
}