#![cfg_attr(test, feature(test))]

use std::hash::Hash;

pub mod cross_linked;
#[cfg(test)]
mod naive;
#[cfg(test)]
mod semi_linked;

pub use cross_linked::CrossLinkedStore;

/// A stable index of a collection of tags in a [`Store`]. As long as an entry
/// is not removed from a [`Store`], other insertions and removals will not
/// invalidate the index nor make it point to a different entry. After removing
/// the entry, there are no guarantees about what the index will point to and
/// should not be used anymore.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntryId(usize);

/// Represents a data structure which stores a list of _entries_, identified by
/// stable indices represented as [`EntryId`], each associated with any number
/// of _tags_. These tags are identified by values of type `K` and hold
/// arbitrary homogenous data of type `V`. More than one tag with the same key
/// `K` can be associated with a single entry.
///
/// `E` represents additional data stored alongside each entry. This allows
/// reusing the internal storage of the [`Store`] instead of using a "sidecar"
/// `BTreeMap<EntryId, E>`.
pub trait Store<K, V, E = ()> {
    /// Returns the number of entries in this store
    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn insert_entry(&mut self, _: E) -> EntryId;
    fn insert_tag(&mut self, _: EntryId, _: K, _: V);
    fn remove_entry(&mut self, _: EntryId) -> E;
    /// Removes all tags associated with the entry. Equivalent to removing and
    /// inserting the entry, but preserves the index.
    fn clear_entry(&mut self, _: EntryId);

    fn entry_data(&self, _: EntryId) -> &E;
    fn entry_data_mut(&mut self, _: EntryId) -> &mut E;

    /// Returns an iterator over all entries in this store
    fn entries<'a>(&'a self) -> impl Iterator<Item = (EntryId, &'a E)>
    where
        E: 'a;

    /// Returns an iterator over mutable references to all entries in this store
    fn entries_mut<'a>(&'a mut self) -> impl Iterator<Item = (EntryId, &'a mut E)>
    where
        E: 'a;

    // NOTE: No default implementation to avoid lifetime bounds
    /// Returns an iterator over the IDs of all entries in this store
    fn entry_ids(&self) -> impl Iterator<Item = EntryId>;

    /// Returns an iterator over all unique tag keys in this store
    fn keys<'a>(&'a self) -> impl Iterator<Item = &'a K>
    where
        K: 'a;

    /// Returns an iterator over tags associated with an entry
    fn tags_by_entry<'a>(&'a self, _: EntryId) -> impl Iterator<Item = (&'a K, &'a V)>
    where
        K: 'a,
        V: 'a;

    /// Returns an iterator over tags with the given key and the associated
    /// entry IDs
    fn tags_by_key<'a>(&'a self, _: &K) -> impl Iterator<Item = (EntryId, &'a V)>
    where
        V: 'a;

    fn insert_entry_with(&mut self, data: E, tags: impl IntoIterator<Item = (K, V)>) -> EntryId {
        let id = self.insert_entry(data);
        for (k, v) in tags {
            self.insert_tag(id, k, v);
        }
        id
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        Store, cross_linked::CrossLinkedStore, naive::NaiveStore, semi_linked::SemiLinkedStore,
    };

    fn test_store(mut store: impl Store<&'static str, i32, &'static str>) {
        let e1 = store.insert_entry_with("e1", [("foo", 1), ("bar", 2)]);
        let e2 = store.insert_entry_with("e2", [("foo", 3)]);
        let e3 = store.insert_entry_with("e3", [("foo", -1), ("bar", -2)]);
        let e4 = store.insert_entry_with("e4", [("bar", 4)]);

        assert_eq!(store.len(), 4);

        store.remove_entry(e3);
        let e5 = store.insert_entry_with("e5", [("baz", 5)]);

        assert_eq!(store.len(), 4);

        assert_eq!(*store.entry_data(e1), "e1");
        assert_eq!(*store.entry_data(e2), "e2");
        assert_eq!(*store.entry_data(e4), "e4");
        assert_eq!(*store.entry_data(e5), "e5");

        assert_eq!(
            store.entries().collect::<Vec<_>>(),
            [(e1, &"e1"), (e2, &"e2"), (e4, &"e4"), (e5, &"e5")]
        );
        assert_eq!(
            store.keys().copied().collect::<Vec<_>>(),
            ["foo", "bar", "baz"]
        );

        assert_eq!(
            store.tags_by_entry(e1).collect::<Vec<_>>(),
            [(&"foo", &1), (&"bar", &2)]
        );
        assert_eq!(store.tags_by_entry(e2).collect::<Vec<_>>(), [(&"foo", &3)]);
        assert_eq!(store.tags_by_entry(e4).collect::<Vec<_>>(), [(&"bar", &4)]);
        assert_eq!(store.tags_by_entry(e5).collect::<Vec<_>>(), [(&"baz", &5)]);

        assert_eq!(
            store.tags_by_key(&"foo").collect::<Vec<_>>(),
            [(e1, &1), (e2, &3)]
        );
        assert_eq!(
            store.tags_by_key(&"bar").collect::<Vec<_>>(),
            [(e1, &2), (e4, &4)]
        );
        assert_eq!(store.tags_by_key(&"baz").collect::<Vec<_>>(), [(e5, &5)]);

        store.remove_entry(e2);

        assert_eq!(store.entry_ids().collect::<Vec<_>>(), [e1, e4, e5]);

        store.clear_entry(e1);

        assert_eq!(store.tags_by_entry(e1).next(), None);
    }

    #[test]
    fn naive() {
        test_store(NaiveStore::default());
    }

    #[test]
    fn cross_linked() {
        test_store(CrossLinkedStore::default());
    }

    #[test]
    fn semi_linked() {
        test_store(SemiLinkedStore::default());
    }

    #[test]
    fn cross_linked_remove_cross_axis() {
        let mut store = CrossLinkedStore::default();

        let e1 = store.insert_entry_with((), [("foo", 1), ("bar", 2)]);
        let e2 = store.insert_entry_with((), [("foo", 3)]);
        let e3 = store.insert_entry_with((), [("foo", 4), ("bar", 5)]);
        let e4 = store.insert_entry_with((), [("bar", 6)]);

        store.remove_entry(e1);
        store.remove_entry(e2);
        store.remove_entry(e4);

        assert_eq!(
            store.tags_by_entry(e3).collect::<Vec<_>>(),
            [(&"foo", &4), (&"bar", &5)]
        );
        assert_eq!(store.tags_by_key(&"foo").collect::<Vec<_>>(), [(e3, &4)]);
    }
}

#[cfg(test)]
mod benches {
    extern crate test;

    use rand::{
        RngExt,
        seq::{IndexedRandom, IteratorRandom},
    };
    use test::Bencher;

    use crate::{
        Store, cross_linked::CrossLinkedStore, naive::NaiveStore, semi_linked::SemiLinkedStore,
    };

    fn populate_store(store: &mut impl Store<String, String>) {
        // Optionally reduce the size of test fixtures to speed up testing
        let (num_entries, num_tags_per_entry) = if cfg!(feature = "lightweight-benchmarks") {
            (100, 10)
        } else {
            (100_000, 100)
        };

        let tag_keys: Vec<_> = (0..1000).map(|_| random_string()).collect();

        for _ in 0..num_entries {
            let entry = store.insert_entry(());

            for _ in 0..num_tags_per_entry {
                store.insert_tag(
                    entry,
                    tag_keys.choose(&mut rand::rng()).unwrap().clone(),
                    random_string(),
                );
            }

            // Remove a random entry every once in a while to simulate a more
            // realistic layout
            if rand::rng().random_bool(0.1) {
                store.remove_entry(store.entry_ids().choose(&mut rand::rng()).unwrap());
            }
        }
    }

    fn random_string() -> String {
        unsafe {
            String::from_utf8_unchecked(
                rand::rng()
                    .sample_iter(rand::distr::Alphanumeric)
                    .take(16)
                    .collect::<Vec<u8>>(),
            )
        }
    }

    #[bench]
    fn naive_search(b: &mut Bencher) {
        let mut store = NaiveStore::default();
        populate_store(&mut store);

        let search_tag = store.keys().choose(&mut rand::rng()).unwrap();

        b.iter(|| {
            for x in store.tags_by_key(search_tag) {
                std::hint::black_box(x);
            }
        });
    }

    #[bench]
    fn cross_linked_search(b: &mut Bencher) {
        let mut store = CrossLinkedStore::default();
        populate_store(&mut store);

        let search_tag = store.keys().choose(&mut rand::rng()).unwrap();

        b.iter(|| {
            for x in store.tags_by_key(search_tag) {
                std::hint::black_box(x);
            }
        });
    }

    #[bench]
    fn semi_linked_search(b: &mut Bencher) {
        let mut store = SemiLinkedStore::default();
        populate_store(&mut store);

        let search_tag = store.keys().choose(&mut rand::rng()).unwrap();

        b.iter(|| {
            for x in store.tags_by_key(search_tag) {
                std::hint::black_box(x);
            }
        });
    }

    #[bench]
    fn naive_iter(b: &mut Bencher) {
        let mut store = NaiveStore::default();
        populate_store(&mut store);

        b.iter(|| {
            for x in store.entries() {
                std::hint::black_box(x);
            }
        });
    }

    #[bench]
    fn cross_linked_iter(b: &mut Bencher) {
        let mut store = CrossLinkedStore::default();
        populate_store(&mut store);

        b.iter(|| {
            for x in store.entries() {
                std::hint::black_box(x);
            }
        });
    }

    #[bench]
    fn semi_linked_iter(b: &mut Bencher) {
        let mut store = SemiLinkedStore::default();
        populate_store(&mut store);

        b.iter(|| {
            for x in store.entries() {
                std::hint::black_box(x);
            }
        });
    }

    fn bench_insertion(b: &mut Bencher, mut store: impl Store<String, String>) {
        let entry_ids = store.entry_ids().collect::<Vec<_>>();
        let tag_keys = store.keys().cloned().collect::<Vec<_>>();

        b.iter(|| {
            let entry = *entry_ids.choose(&mut rand::rng()).unwrap();
            let key = tag_keys.choose(&mut rand::rng()).unwrap().clone();

            store.insert_tag(entry, key, random_string());
        });
    }

    #[bench]
    fn naive_insert(b: &mut Bencher) {
        let mut store = NaiveStore::default();
        populate_store(&mut store);
        bench_insertion(b, store);
    }

    #[bench]
    fn cross_linked_insert(b: &mut Bencher) {
        let mut store = CrossLinkedStore::default();
        populate_store(&mut store);
        bench_insertion(b, store);
    }

    #[bench]
    fn semi_linked_insert(b: &mut Bencher) {
        let mut store = SemiLinkedStore::default();
        populate_store(&mut store);
        bench_insertion(b, store);
    }

    fn bench_removal(b: &mut Bencher, mut store: impl Store<String, String>) {
        let mut entry_ids = store.entry_ids().collect::<Vec<_>>();

        b.iter(|| {
            // FIXME: This should not be part of the benchmark, but I don't know
            // how to otherwise ensure there are always entries to remove
            if entry_ids.is_empty() {
                populate_store(&mut store);
                entry_ids = store.entry_ids().collect::<Vec<_>>();
            }

            let (i, &entry) = entry_ids
                .iter()
                .enumerate()
                .choose(&mut rand::rng())
                .unwrap();

            store.remove_entry(entry);
            entry_ids.swap_remove(i);
        });
    }

    #[bench]
    fn naive_remove(b: &mut Bencher) {
        let mut store = NaiveStore::default();
        populate_store(&mut store);
        bench_removal(b, store);
    }

    #[bench]
    fn cross_linked_remove(b: &mut Bencher) {
        let mut store = CrossLinkedStore::default();
        populate_store(&mut store);
        bench_removal(b, store);
    }

    #[bench]
    fn semi_linked_remove(b: &mut Bencher) {
        let mut store = SemiLinkedStore::default();
        populate_store(&mut store);
        bench_removal(b, store);
    }
}
