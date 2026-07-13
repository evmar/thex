#![allow(unused)]

use std::collections::{HashMap, HashSet};

pub struct Union<T>(HashMap<T, T>);

impl<T> Union<T>
where
    T: Eq + std::hash::Hash + Ord + Copy + std::fmt::Debug,
{
    pub fn new() -> Self {
        Union(HashMap::new())
    }

    pub fn join(&mut self, a: T, b: T) {
        let top = self.find(a).min(self.find(b));
        self.0.insert(a, top);
        self.0.insert(b, top);
    }

    pub fn find(&mut self, a: T) -> T {
        if let Some(a) = self.lookup(a) {
            return a;
        }
        self.0.insert(a, a);
        return a;
    }

    pub fn lookup(&self, a: T) -> Option<T> {
        let mut a = a;
        loop {
            let next = self.0.get(&a)?;
            if *next == a {
                return Some(a);
            }
            a = *next;
        }
    }

    #[allow(dead_code)] // useful in debugging
    pub fn sets(&mut self) -> Vec<HashSet<T>> {
        let mut sets = HashMap::<T, HashSet<T>>::new();
        for a in self.0.keys() {
            let set = sets.entry(self.lookup(*a).unwrap()).or_default();
            set.insert(*a);
        }
        sets.into_values().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_union() {
        let mut u = Union::new();
        u.join(1, 2);
        u.join(2, 3);
        assert_eq!(u.find(1), u.find(2));
        assert_eq!(u.find(2), u.find(3));
        assert_eq!(u.find(1), 1);
        assert_eq!(u.find(2), 1);
        assert_eq!(u.find(3), 1);
        u.join(4, 5);
        assert_eq!(u.find(4), u.find(5));
        assert_ne!(u.find(1), u.find(4));
    }
}
