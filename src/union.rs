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
        if self.0.get(&a).is_none() {
            self.0.insert(a, a);
            return a;
        }
        self.lookup(a)
    }

    fn lookup(&self, a: T) -> T {
        let mut a = a;
        loop {
            println!("find {a:?}");
            let next = self.0.get(&a).unwrap();
            if *next == a {
                return a;
            }
            a = *next;
        }
    }

    pub fn sets(&mut self) -> Vec<HashSet<T>> {
        let mut sets = HashMap::<T, HashSet<T>>::new();
        for a in self.0.keys() {
            let set = sets.entry(self.lookup(*a)).or_default();
            set.insert(*a);
        }
        sets.into_values().collect()
    }
}
