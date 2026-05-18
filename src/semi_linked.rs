use std::hash::Hash;

use indexmap::IndexMap;
use stable_vec::StableVec;

use crate::{EntryId, Store};

/// Alternative implementation using a vec-of-vecs and a set of linked lists
/// across tags only. It seems to fare slightly worse than [`SparseStore`]
/// in benchmarks.
pub struct SemiLinkedStore<K, V, E = ()> {
    entries: StableVec<Entry<V, E>>,
    key_lists: IndexMap<K, TagList>,
}

struct Entry<V, E> {
    data: E,
    tags: Vec<Tag<V>>,
}

struct Tag<V> {
    value: V,
    key_index: usize,
    prev: Option<(usize, usize)>,
    next: Option<(usize, usize)>,
}

#[derive(Default)]
enum TagList {
    #[default]
    Empty,
    NonEmpty {
        first: (usize, usize),
        last: (usize, usize),
    },
}

impl TagList {
    fn first(&self) -> Option<(usize, usize)> {
        match self {
            Self::Empty => None,
            Self::NonEmpty { first, .. } => Some(*first),
        }
    }

    fn append<V, E>(&mut self, entries: &mut StableVec<Entry<V, E>>, indices: (usize, usize)) {
        match self {
            Self::Empty => {
                *self = Self::NonEmpty {
                    first: indices,
                    last: indices,
                };
            }
            Self::NonEmpty { last, .. } => {
                entries[last.0].tags[last.1].next = Some(indices);
                entries[indices.0].tags[indices.1].prev = Some(*last);
                *last = indices;
            }
        }
    }

    fn remove<V, E>(&mut self, entries: &mut StableVec<Entry<V, E>>, indices: (usize, usize)) {
        match self {
            Self::Empty => panic!("attempted to remove from empty list"),
            Self::NonEmpty { first, last } => {
                if indices == *first && indices == *last {
                    *self = TagList::Empty;
                    return;
                }

                {
                    let tag = &entries[indices.0].tags[indices.1];

                    if indices == *first {
                        *first = tag.next.unwrap();
                    } else {
                        let prev = tag.prev.unwrap();
                        entries[prev.0].tags[prev.1].next = tag.next;
                    }
                }

                {
                    let tag = &entries[indices.0].tags[indices.1];

                    if indices == *last {
                        *last = tag.prev.unwrap();
                    } else {
                        let next = tag.next.unwrap();
                        entries[next.0].tags[next.1].prev = tag.prev;
                    }
                }
            }
        }
    }
}

impl<K, V, E> Default for SemiLinkedStore<K, V, E> {
    fn default() -> Self {
        Self {
            entries: Default::default(),
            key_lists: Default::default(),
        }
    }
}

impl<K: Hash + Eq, V, E> Store<K, V, E> for SemiLinkedStore<K, V, E> {
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
        let key_list_entry = self.key_lists.entry(k);
        let key_index = key_list_entry.index();
        let key_list = key_list_entry.or_default();

        let entry = &mut self.entries[id.0];
        let tag_index = entry.tags.len();
        entry.tags.push(Tag {
            value: v,
            key_index,
            prev: None,
            next: None,
        });

        key_list.append(&mut self.entries, (id.0, tag_index));
    }

    fn remove_entry(&mut self, id: EntryId) -> E {
        self.clear_entry(id);
        self.entries.remove(id.0).unwrap().data
    }

    fn clear_entry(&mut self, id: EntryId) {
        for tag_index in 0..self.entries[id.0].tags.len() {
            let tag = &self.entries[id.0].tags[tag_index];
            let (_, key_list) = self.key_lists.get_index_mut(tag.key_index).unwrap();
            key_list.remove(&mut self.entries, (id.0, tag_index));
        }

        self.entries[id.0].tags.clear();
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
        self.key_lists.keys()
    }

    fn tags_by_entry<'a>(&'a self, id: EntryId) -> impl Iterator<Item = (&'a K, &'a V)>
    where
        K: 'a,
        V: 'a,
    {
        self.entries[id.0]
            .tags
            .iter()
            .map(|t| (self.key_lists.get_index(t.key_index).unwrap().0, &t.value))
    }

    fn tags_by_key<'a>(&'a self, k: &K) -> impl Iterator<Item = (EntryId, &'a V)>
    where
        V: 'a,
    {
        Iter {
            store: self,
            indices: self.key_lists[k].first(),
        }
        .map(|indices| {
            let tag = &self.entries[indices.0].tags[indices.1];
            (EntryId(indices.0), &tag.value)
        })
    }
}

struct Iter<'a, K, V, E> {
    store: &'a SemiLinkedStore<K, V, E>,
    indices: Option<(usize, usize)>,
}

impl<'a, K, V, E> Iterator for Iter<'a, K, V, E> {
    type Item = (usize, usize);

    fn next(&mut self) -> Option<Self::Item> {
        let indices = self.indices?;
        self.indices = self.store.entries[indices.0].tags[indices.1].next;
        Some(indices)
    }
}
