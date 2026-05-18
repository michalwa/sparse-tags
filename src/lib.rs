#![cfg_attr(test, feature(test))]

use std::hash::Hash;

#[cfg(test)]
mod naive;
pub mod sparse;

pub use sparse::SparseStore;

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
pub trait Store<K, V> {
    /// Returns the number of entries in this store
    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn insert_entry(&mut self) -> EntryId;
    fn insert_tag(&mut self, _: EntryId, _: K, _: V);
    fn remove_entry(&mut self, _: EntryId);
    /// Removes all tags associated with the entry. Equivalent to removing and
    /// inserting the entry, but preserves the index.
    fn clear_entry(&mut self, _: EntryId);

    /// Returns an iterator over all entry IDs in this store
    fn entries(&self) -> impl Iterator<Item = EntryId>;

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

    fn insert_entry_with(&mut self, tags: impl IntoIterator<Item = (K, V)>) -> EntryId {
        let id = self.insert_entry();
        for (k, v) in tags {
            self.insert_tag(id, k, v);
        }
        id
    }
}

#[cfg(test)]
mod tests {
    use crate::{Store, naive::NaiveStore, sparse::SparseStore};

    fn test_store(mut store: impl Store<&'static str, i32>) {
        let e1 = store.insert_entry_with([("foo", 1), ("bar", 2)]);
        let e2 = store.insert_entry_with([("foo", 3)]);
        let e3 = store.insert_entry_with([("foo", -1), ("bar", -2)]);
        let e4 = store.insert_entry_with([("bar", 4)]);

        assert_eq!(store.len(), 4);

        store.remove_entry(e3);
        let e5 = store.insert_entry_with([("baz", 5)]);

        assert_eq!(store.len(), 4);

        assert_eq!(store.entries().collect::<Vec<_>>(), [e1, e2, e4, e5]);
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

        assert_eq!(store.entries().collect::<Vec<_>>(), [e1, e4, e5]);

        store.clear_entry(e1);

        assert_eq!(store.tags_by_entry(e1).next(), None);
    }

    #[test]
    fn naive() {
        test_store(NaiveStore::default());
    }

    #[test]
    fn sparse() {
        test_store(SparseStore::default());
    }

    #[test]
    fn sparse_remove_cross_axis() {
        let mut store = SparseStore::default();

        let e1 = store.insert_entry_with([("foo", 1), ("bar", 2)]);
        let e2 = store.insert_entry_with([("foo", 3)]);
        let e3 = store.insert_entry_with([("foo", 4), ("bar", 5)]);
        let e4 = store.insert_entry_with([("bar", 6)]);

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

    use crate::{Store, naive::NaiveStore, sparse::SparseStore};

    fn populate_store(store: &mut impl Store<String, String>) {
        // Optionally reduce the size of test fixtures to speed up testing
        let (num_entries, num_tags_per_entry) = if cfg!(feature = "lightweight-benchmarks") {
            (100, 10)
        } else {
            (100_000, 100)
        };

        let tag_keys: Vec<_> = (0..1000).map(|_| random_string()).collect();

        for _ in 0..num_entries {
            let entry = store.insert_entry();

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
                store.remove_entry(store.entries().choose(&mut rand::rng()).unwrap());
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
    fn sparse_search(b: &mut Bencher) {
        let mut store = SparseStore::new();
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
    fn sparse_iter(b: &mut Bencher) {
        let mut store = SparseStore::new();
        populate_store(&mut store);

        b.iter(|| {
            for x in store.entries() {
                std::hint::black_box(x);
            }
        });
    }
}
