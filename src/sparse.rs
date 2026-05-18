use std::{hash::Hash, marker::PhantomData};

use indexmap::IndexMap;
use slab::Slab;
use stable_vec::StableVec;

use crate::{EntryId, Store};

/// A 2-axis doubly-linked list implementation of a [`Store`]. Refer to
/// [`Store`] for more general documentation.
pub struct SparseStore<K, V> {
    /// Uses `StableVec` instead of `Slab` to preserve the insertion order
    entry_lists: StableVec<NodeList<EntryAxis>>,
    key_lists: IndexMap<K, NodeList<KeyAxis>>,
    nodes: Slab<Node<V>>,
}

struct Node<V> {
    entry: EntryId,
    key_index: usize,
    value: V,
    /// Indexed by [`EntryAxis::INDEX`] and [`KeyAxis::INDEX`]
    links: [Links; 2],
}

impl<V> Node<V> {
    fn new(entry: EntryId, key_index: usize, value: V) -> Self {
        Self {
            entry,
            key_index,
            value,
            links: Default::default(),
        }
    }
}

trait Axis {
    const INDEX: usize;
}

struct EntryAxis;

impl Axis for EntryAxis {
    const INDEX: usize = 0;
}

struct KeyAxis;

impl Axis for KeyAxis {
    const INDEX: usize = 1;
}

#[derive(Default, Clone, Copy)]
struct Links {
    prev: Option<usize>,
    next: Option<usize>,
}

enum NodeList<A: Axis> {
    Empty,
    NonEmpty {
        _marker: PhantomData<A>,
        first: usize,
        last: usize,
    },
}

impl<A: Axis> NodeList<A> {
    fn first(&self) -> Option<usize> {
        match self {
            Self::Empty => None,
            Self::NonEmpty { first, .. } => Some(*first),
        }
    }

    fn append<V>(&mut self, nodes: &mut Slab<Node<V>>, node: usize) {
        match self {
            Self::Empty => {
                *self = Self::NonEmpty {
                    _marker: PhantomData,
                    first: node,
                    last: node,
                };
            }
            Self::NonEmpty { last, .. } => {
                nodes[node].links[A::INDEX].prev = Some(*last);
                nodes[*last].links[A::INDEX].next = Some(node);
                *last = node;
            }
        }
    }

    fn remove<V>(&mut self, nodes: &mut Slab<Node<V>>, node: usize) {
        match self {
            Self::Empty => panic!("attempted to remove from an empty list"),
            Self::NonEmpty { first, last, .. } => {
                let links = nodes.remove(node).links[A::INDEX];

                if node == *first && node == *last {
                    *self = NodeList::Empty;
                    return;
                }

                if node == *first {
                    *first = links.next.unwrap();
                } else {
                    nodes[links.prev.unwrap()].links[A::INDEX].next = links.next;
                }

                if node == *last {
                    *last = links.prev.unwrap();
                } else {
                    nodes[links.next.unwrap()].links[A::INDEX].prev = links.prev;
                }
            }
        }
    }
}

impl NodeList<EntryAxis> {
    fn clear_entry<K, V>(
        &mut self,
        nodes: &mut Slab<Node<V>>,
        key_lists: &mut IndexMap<K, NodeList<KeyAxis>>,
    ) {
        let mut next = self.first();
        *self = Self::Empty;

        while let Some(node) = next {
            next = nodes[node].links[EntryAxis::INDEX].next;

            let (_, key_list) = key_lists.get_index_mut(nodes[node].key_index).unwrap();
            key_list.remove(nodes, node);
        }
    }
}

impl<K, V> Default for SparseStore<K, V> {
    fn default() -> Self {
        Self {
            entry_lists: StableVec::new(),
            key_lists: IndexMap::new(),
            nodes: Slab::new(),
        }
    }
}

impl<K, V> SparseStore<K, V> {
    pub fn new() -> Self {
        Self::default()
    }
}

impl<K: Hash + Eq, V> Store<K, V> for SparseStore<K, V> {
    fn len(&self) -> usize {
        self.entry_lists.num_elements()
    }

    fn is_empty(&self) -> bool {
        self.entry_lists.is_empty()
    }

    fn insert_entry(&mut self) -> EntryId {
        EntryId(self.entry_lists.push(NodeList::Empty))
    }

    fn insert_tag(&mut self, id: EntryId, k: K, v: V) {
        let key_entry = self.key_lists.entry(k);
        let key_index = key_entry.index();
        let nodes_by_key = key_entry.or_insert(NodeList::Empty);

        let nodes_by_entry = &mut self.entry_lists[id.0];

        let node = self.nodes.insert(Node::new(id, key_index, v));
        nodes_by_entry.append(&mut self.nodes, node);
        nodes_by_key.append(&mut self.nodes, node);
    }

    fn remove_entry(&mut self, id: EntryId) {
        self.clear_entry(id);
        self.entry_lists.remove(id.0);
    }

    fn clear_entry(&mut self, id: EntryId) {
        self.entry_lists[id.0].clear_entry(&mut self.nodes, &mut self.key_lists);
    }

    fn entries(&self) -> impl Iterator<Item = EntryId> {
        self.entry_lists.iter().map(|(i, _)| EntryId(i))
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
        Iter::<_, _, EntryAxis> {
            _marker: PhantomData,
            store: self,
            index: self.entry_lists[id.0].first(),
        }
        .map(|i| {
            let node = &self.nodes[i];
            let (key, _) = self.key_lists.get_index(node.key_index).unwrap();
            (key, &node.value)
        })
    }

    fn tags_by_key<'a>(&'a self, k: &K) -> impl Iterator<Item = (EntryId, &'a V)>
    where
        V: 'a,
    {
        Iter::<_, _, KeyAxis> {
            _marker: PhantomData,
            store: self,
            index: self.key_lists[k].first(),
        }
        .map(|i| {
            let node = &self.nodes[i];
            (node.entry, &node.value)
        })
    }
}

struct Iter<'a, K, V, A: Axis> {
    _marker: PhantomData<A>,
    store: &'a SparseStore<K, V>,
    index: Option<usize>,
}

impl<'a, K, V, A: Axis> Iterator for Iter<'a, K, V, A> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        let index = self.index?;
        self.index = self.store.nodes[index].links[A::INDEX].next;
        Some(index)
    }
}
