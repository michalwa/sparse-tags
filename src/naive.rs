use std::hash::Hash;

use itertools::Itertools;
use stable_vec::StableVec;

use crate::{EntryId, Store};

/// A naive reference implementation using linear scans, for benchmarking only
#[derive(Default)]
pub struct NaiveStore<K, V, E = ()> {
    entries: StableVec<(E, Vec<(K, V)>)>,
}

// NOTE: Because of the linear scan, `tags_by_key` must borrow the key to perform
// equality checks. However, `Store` explicitly forbids this to give users looser
// lifetime bounds. Therefore, to implement this method, `NativeStore` requires
// the key to be `Clone`. This is very suboptimal, but it doesn't matter for this
// implementation.
impl<K: Hash + Eq + Clone, V, E> Store<K, V, E> for NaiveStore<K, V, E> {
    fn len(&self) -> usize {
        self.entries.num_elements()
    }

    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn insert_entry(&mut self, data: E) -> EntryId {
        EntryId(self.entries.push((data, Vec::new())))
    }

    fn insert_tag(&mut self, id: EntryId, k: K, v: V) {
        self.entries[id.0].1.push((k, v));
    }

    fn remove_entry(&mut self, id: EntryId) -> E {
        self.entries.remove(id.0).unwrap().0
    }

    fn clear_entry(&mut self, id: EntryId) {
        self.entries[id.0].1.clear();
    }

    fn entry_exists(&self, id: EntryId) -> bool {
        self.entries.has_element_at(id.0)
    }

    fn entry_data(&self, id: EntryId) -> &E {
        &self.entries[id.0].0
    }

    fn entry_data_mut(&mut self, id: EntryId) -> &mut E {
        &mut self.entries[id.0].0
    }

    fn entries<'a>(&'a self) -> impl Iterator<Item = (EntryId, &'a E)>
    where
        E: 'a,
    {
        self.entries
            .iter()
            .map(|(id, (data, _))| (EntryId(id), data))
    }

    fn entries_mut<'a>(&'a mut self) -> impl Iterator<Item = (EntryId, &'a mut E)>
    where
        E: 'a,
    {
        self.entries
            .iter_mut()
            .map(|(id, (data, _))| (EntryId(id), data))
    }

    fn entry_ids(&self) -> impl Iterator<Item = EntryId> {
        self.entries.indices().map(EntryId)
    }

    fn keys<'a>(&'a self) -> impl Iterator<Item = &'a K>
    where
        K: 'a,
    {
        self.entries
            .values()
            .flat_map(|(_, ts)| ts.iter().map(|(k, _)| k))
            .unique()
    }

    fn tags<'a>(&'a self) -> impl Iterator<Item = (EntryId, &'a K, &'a V)>
    where
        K: 'a,
        V: 'a,
    {
        self.entries
            .iter()
            .flat_map(|(id, entry)| entry.1.iter().map(move |(k, v)| (EntryId(id), k, v)))
    }

    fn tags_mut<'a>(&'a mut self) -> impl Iterator<Item = (EntryId, &'a K, &'a mut V)>
    where
        K: 'a,
        V: 'a,
    {
        self.entries
            .iter_mut()
            .flat_map(|(id, entry)| entry.1.iter_mut().map(move |(k, v)| (EntryId(id), &*k, v)))
    }

    fn tags_by_entry<'a>(&'a self, id: EntryId) -> impl Iterator<Item = (&'a K, &'a V)>
    where
        K: 'a,
        V: 'a,
    {
        self.entries[id.0].1.iter().map(|(k, v)| (k, v))
    }

    fn tags_by_key<'a>(
        &'a self,
        key: &K,
    ) -> impl Iterator<Item = (EntryId, &'a V)> + use<'a, K, V, E>
    where
        V: 'a,
    {
        let key = key.clone();

        self.entries.iter().flat_map(move |(id, (_, ts))| {
            let key = key.clone();
            ts.iter()
                .filter(move |(k, _)| *k == key)
                .map(move |(_, v)| (EntryId(id), v))
        })
    }
}
