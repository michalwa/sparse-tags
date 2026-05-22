use std::{hash::Hash, marker::PhantomData};

use indexmap::IndexMap;
use slab::Slab;
use stable_vec::StableVec;

use crate::{EntryId, Store};

/// A multi-linked list implementation of a [`Store`]
///
/// Fares significantly worse than [`IndexedStore`], especially in search, but
/// kept for reference and because it's cool-looking (:
///
/// Internally represented as a graph of nodes, each of which represents an
/// instance of a tag associated with an entry, and stores double-linked
/// pointers in 2 axes: the entry chain and tag chain. The entry chain connects
/// nodes which share the same [`EntryId`] and the tag chains (one for each key
/// `K`) connect nodes with the same tag key `K`.
///
/// This implementation effectively has the same time complexities as
/// [`IndexedStore`] and my guess for why it ends up doing worse is worse cache
/// locality.
pub struct MultiLinkedStore<K, V, E = ()> {
    /// Uses `StableVec` instead of `Slab` to preserve the insertion order
    entries: StableVec<Entry<E>>,
    key_lists: IndexMap<K, NodeList<KeyAxis>>,
    nodes: Slab<Node<V>>,
}

struct Entry<E> {
    data: E,
    nodes: NodeList<EntryAxis>,
}

impl<E> Entry<E> {
    fn new(data: E) -> Self {
        Self {
            data,
            nodes: NodeList::Empty,
        }
    }
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

impl<K, V, E> Default for MultiLinkedStore<K, V, E> {
    fn default() -> Self {
        Self {
            entries: StableVec::new(),
            key_lists: IndexMap::new(),
            nodes: Slab::new(),
        }
    }
}

impl<K: Hash + Eq, V, E> Store<K, V, E> for MultiLinkedStore<K, V, E> {
    fn len(&self) -> usize {
        self.entries.num_elements()
    }

    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn insert_entry(&mut self, data: E) -> EntryId {
        EntryId(self.entries.push(Entry::new(data)))
    }

    fn insert_tag(&mut self, id: EntryId, k: K, v: V) {
        let key_entry = self.key_lists.entry(k);
        let key_index = key_entry.index();
        let nodes_by_key = key_entry.or_insert(NodeList::Empty);

        let nodes_by_entry = &mut self.entries[id.0].nodes;

        let node = self.nodes.insert(Node::new(id, key_index, v));
        nodes_by_entry.append(&mut self.nodes, node);
        nodes_by_key.append(&mut self.nodes, node);
    }

    fn remove_entry(&mut self, id: EntryId) -> E {
        self.clear_entry(id);
        self.entries.remove(id.0).unwrap().data
    }

    fn clear_entry(&mut self, id: EntryId) {
        self.entries[id.0]
            .nodes
            .clear_entry(&mut self.nodes, &mut self.key_lists);
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
        self.entries.iter().map(|(i, e)| (EntryId(i), &e.data))
    }

    fn entries_mut<'a>(&'a mut self) -> impl Iterator<Item = (EntryId, &'a mut E)>
    where
        E: 'a,
    {
        self.entries
            .iter_mut()
            .map(|(i, e)| (EntryId(i), &mut e.data))
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

    fn tags<'a>(&'a self) -> impl Iterator<Item = (EntryId, &'a K, &'a V)>
    where
        K: 'a,
        V: 'a,
    {
        self.nodes.iter().map(|(_, node)| {
            let (key, _) = self.key_lists.get_index(node.key_index).unwrap();
            (node.entry, key, &node.value)
        })
    }

    fn tags_mut<'a>(&'a mut self) -> impl Iterator<Item = (EntryId, &'a K, &'a mut V)>
    where
        K: 'a,
        V: 'a,
    {
        self.nodes.iter_mut().map(|(_, node)| {
            let (key, _) = self.key_lists.get_index(node.key_index).unwrap();
            (node.entry, key, &mut node.value)
        })
    }

    fn tags_by_entry<'a>(&'a self, id: EntryId) -> impl Iterator<Item = (&'a K, &'a V)>
    where
        K: 'a,
        V: 'a,
    {
        Iter::<_, _, _, EntryAxis> {
            _marker: PhantomData,
            store: self,
            index: self.entries[id.0].nodes.first(),
        }
        .map(|i| {
            let node = &self.nodes[i];
            let (key, _) = self.key_lists.get_index(node.key_index).unwrap();
            (key, &node.value)
        })
    }

    fn tags_by_key<'a>(&'a self, k: &K) -> impl Iterator<Item = (EntryId, &'a V)> + use<'a, K, V, E>
    where
        V: 'a,
    {
        self.key_lists.get(k).into_iter().flat_map(|list| {
            Iter::<_, _, _, KeyAxis> {
                _marker: PhantomData,
                store: self,
                index: list.first(),
            }
            .map(|i| {
                let node = &self.nodes[i];
                (node.entry, &node.value)
            })
        })
    }
}

struct Iter<'a, K, V, E, A: Axis> {
    _marker: PhantomData<A>,
    store: &'a MultiLinkedStore<K, V, E>,
    index: Option<usize>,
}

impl<'a, K, V, E, A: Axis> Iterator for Iter<'a, K, V, E, A> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        let index = self.index?;
        self.index = self.store.nodes[index].links[A::INDEX].next;
        Some(index)
    }
}
