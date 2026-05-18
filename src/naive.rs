use std::{collections::BTreeMap, hash::Hash};

use itertools::Itertools;

use crate::{EntryId, Store};

/// A naive reference implementation involving maps and linear scans, for
/// benchmarking only
#[derive(Default)]
pub struct NaiveStore<K, V> {
    next_entry_id: usize,
    entries: BTreeMap<usize, Vec<(K, V)>>,
}

impl<K: Hash + Eq, V> Store<K, V> for NaiveStore<K, V> {
    fn len(&self) -> usize {
        self.entries.len()
    }

    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn insert_entry(&mut self) -> EntryId {
        self.entries.insert(self.next_entry_id, Vec::new());
        let id = EntryId(self.next_entry_id);
        self.next_entry_id += 1;
        id
    }

    fn insert_tag(&mut self, id: EntryId, k: K, v: V) {
        self.entries.get_mut(&id.0).unwrap().push((k, v));
    }

    fn remove_entry(&mut self, id: EntryId) {
        self.entries.remove(&id.0);
    }

    fn clear_entry(&mut self, id: EntryId) {
        self.entries.get_mut(&id.0).unwrap().clear();
    }

    fn entries(&self) -> impl Iterator<Item = EntryId> {
        self.entries.keys().copied().map(EntryId)
    }

    fn keys<'a>(&'a self) -> impl Iterator<Item = &'a K>
    where
        K: 'a,
    {
        self.entries
            .values()
            .flat_map(|ts| ts.iter().map(|(k, _)| k))
            .unique()
    }

    fn tags_by_entry<'a>(&'a self, id: EntryId) -> impl Iterator<Item = (&'a K, &'a V)>
    where
        K: 'a,
        V: 'a,
    {
        self.entries[&id.0].iter().map(|(k, v)| (k, v))
    }

    fn tags_by_key<'a>(&'a self, key: &K) -> impl Iterator<Item = (EntryId, &'a V)>
    where
        V: 'a,
    {
        self.entries.iter().flat_map(move |(&id, ts)| {
            ts.iter()
                .filter(move |(k, _)| k == key)
                .map(move |(_, v)| (EntryId(id), v))
        })
    }
}
