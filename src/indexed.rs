use std::hash::Hash;

use indexmap::IndexMap;
use stable_vec::StableVec;

use crate::{EntryId, Store};

/// Implementation of a [`Store`] using a vec-of-vecs and a map of index lists
/// for each key
///
/// * [`Store::insert_entry()`] and [`Store::insert_tag()`] are `O(1)`.
/// * [`Store::remove_entry()`] and [`Store::clear_entry()`] are linear in the
///   number of tags present on the entry.
/// * Iterating [`Store::tags_by_entry()`] and [`Store::tags_by_key()`] is
///   linear in the number of entries or tags, respectively, matching the predicate.
pub struct IndexedStore<K, V, E = ()> {
    entries: StableVec<Entry<V, E>>,
    key_indices: IndexMap<K, StableVec<Index>>,
}

#[derive(Clone, Copy)]
struct Index {
    entry: EntryId,
    tag: usize,
}

struct Entry<V, E> {
    data: E,
    /// Tags cannot be removed individually, so it's fine to use a plain `Vec`
    tags: Vec<Tag<V>>,
}

struct Tag<V> {
    value: V,
    /// Index into `IndexedStore.key_indices`
    key_index: usize,
    /// Index into `IndexedStore.key_indices.get_index(key_index)` for fast removal
    index_index: usize,
}

impl<K, V, E> Default for IndexedStore<K, V, E> {
    fn default() -> Self {
        Self {
            entries: Default::default(),
            key_indices: Default::default(),
        }
    }
}

impl<K, V, E> IndexedStore<K, V, E> {
    pub fn new() -> Self {
        Default::default()
    }
}

impl<K: Hash + Eq, V, E> Store<K, V, E> for IndexedStore<K, V, E> {
    fn len(&self) -> usize {
        self.entries.num_elements()
    }

    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn insert_entry(&mut self, data: E) -> EntryId {
        EntryId(self.entries.push(Entry {
            data,
            tags: Vec::new(),
        }))
    }

    fn insert_tag(&mut self, id: EntryId, k: K, v: V) {
        let key_list_entry = self.key_indices.entry(k);
        let key_index = key_list_entry.index();
        let key_list = key_list_entry.or_default();

        let entry = &mut self.entries[id.0];
        let tag_index = entry.tags.len();

        entry.tags.push(Tag {
            value: v,
            key_index,
            index_index: key_list.push(Index {
                entry: id,
                tag: tag_index,
            }),
        });
    }

    fn remove_entry(&mut self, id: EntryId) -> E {
        self.clear_entry(id);
        self.entries.remove(id.0).unwrap().data
    }

    fn clear_entry(&mut self, id: EntryId) {
        for tag in self.entries[id.0].tags.drain(..) {
            let (_, indices) = self.key_indices.get_index_mut(tag.key_index).unwrap();
            indices.remove(tag.index_index);
        }
    }

    fn entry_data(&self, id: EntryId) -> &E {
        &self.entries[id.0].data
    }

    fn entry_data_mut(&mut self, id: EntryId) -> &mut E {
        &mut self.entries[id.0].data
    }

    fn entries<'a>(&'a self) -> impl Iterator<Item = (EntryId, &'a E)>
    where
        E: 'a,
    {
        self.entries.iter().map(|(id, e)| (EntryId(id), &e.data))
    }

    fn entries_mut<'a>(&'a mut self) -> impl Iterator<Item = (EntryId, &'a mut E)>
    where
        E: 'a,
    {
        self.entries
            .iter_mut()
            .map(|(id, e)| (EntryId(id), &mut e.data))
    }

    fn entry_ids(&self) -> impl Iterator<Item = EntryId> {
        self.entries.indices().map(EntryId)
    }

    fn keys<'a>(&'a self) -> impl Iterator<Item = &'a K>
    where
        K: 'a,
    {
        self.key_indices.keys()
    }

    fn tags_by_entry<'a>(&'a self, id: EntryId) -> impl Iterator<Item = (&'a K, &'a V)>
    where
        K: 'a,
        V: 'a,
    {
        self.entries[id.0]
            .tags
            .iter()
            .map(|t| (self.key_indices.get_index(t.key_index).unwrap().0, &t.value))
    }

    fn tags_by_key<'a>(&'a self, k: &K) -> impl Iterator<Item = (EntryId, &'a V)>
    where
        V: 'a,
    {
        self.key_indices[k].iter().map(|(_, &index)| {
            (
                index.entry,
                &self.entries[index.entry.0].tags[index.tag].value,
            )
        })
    }
}
