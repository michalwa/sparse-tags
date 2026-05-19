# sparse-tags

A study and benchmark of alternative implementations for an ECS-like data structure used to store and efficiently iterate over sets of key-value pairs in Rust.

Developed to find a solution for a search cache for [factbook](https://github.com/michalwa/factbook).

## Design

### Definitions

- A key-value pair is called a _tag_. This corresponds to the notion of a _component_ in ECS.
- A group of key-value pairs is called an _entry_, identified by an _entry ID_. This corresponds to an _entity_ in ECS.
- A _store_ holds arbitrarily many entries and their associated tags.

Unlike in traditional ECS architecture, multiple tags within an entry may share the same key. This means an entry is similar to a multi-map or a `Map<K, Vec<V>>` where `K` is the key type and `V` is the value type, moreso than a `Map<K, V>` (specific map implementation left implicit).

Tag keys and values are currently modelled to be homogenous, so implementing an actual ECS using this crate will not provide adequate compile-time guarantees. Though a future extension to support a functionality similar to [`typemap`](https://github.com/reem/rust-typemap) is possible.

The word _sparse_ is used to mean that not all entries necessarily contain tags of all unique keys, and also that the most interesting, though suboptimal, implementation resembles that of a [multi-linked list sparse matrix](https://webdocs.cs.ualberta.ca/~holte/T26/mlinked-lists.html).

## Implementation

### Naive linear implementation

[`NaiveStore`](src/naive.rs) is arguably an unfairly naive implementation, which simply stores entries as `Vec`s of `(K, V)` pairs, and performs linear scans on each of those `Vec`s on search. Unsurprisingly, it is the worst performer in the case of search.

### Indexed linear implementation

[`IndexedStore`](src/indexed.rs) is a less naive, but still straightforward implementation, and ends up performing the best. It is essentially an extension of the first naive implementation, adding a map of index lists `Map<K, Vec<(usize, usize)>>`. This allows search by tag to simply iterate the respective list of indices. Additionally, to speed up removals (avoid having to scan the index list) tags stored inside entry also hold indices into the index list.

```
      ┌────────────────────────────┐                        
┌─────┼────────────────┐   ┌───────┼───────────────────────┐
│   ┌─▼────────┐       │   │   ┌───▼─────────────────────┐ │
│ 1 │ A1 B1 D1 │       │   │ A │ (0,0) (2,0) (3,0) (4,0) │ │
│   └──────────┘       │   │   └─────────────────────────┘ │
│   ┌───────┐          │   │   ┌─────────────────────────┐ │
│ 2 │ B2 C2 │          │   │ B │ (0,1) (1,0) (2,1) (3,1) │ │
│   └───────┘          │   │   └─────────────────────────┘ │
│   ┌───────┐          │   │   ┌───────────────────┐       │
│ 3 │ A3 B3 │          │   │ C │ (1,1) (3,2) (4,1) │       │
│   └───────┘          │   │   └───────────────────┘       │
│   ┌────────────────┐ │   │   ┌─────────────┐             │
│ 4 │ A4 B4 C4 D4 E4 │ │   │ D │ (0,2) (3,3) │             │
│   └────────────────┘ │   │   └─────────────┘             │
│   ┌──────────┐       │   │   ┌─────────────┐             │
│ 5 │ A5 C5 E5 │       │   │ E │ (3,4) (4,2) │             │
│   └──────────┘       │   │   └─────────────┘             │
└─entries──────────────┘   └─key_indices───────────────────┘
```

_(other links hidden for clarity)_

`insert_entry` and `insert_tag` are `O(1)`.

The downside is that `remove_entry` and `clear_entry` are linear in the number of tags present on the entry, because the index lists need to be updated. Based on the intended use case it is assumed that realistically there will be significantly more entries than keys, and that this is therefore the right tradeoff.

### Multi-linked list implementation

[`MultiLinkedStore`](src/multi_linked.rs) was initially the primary focus of this crate, because to an untrained overengineering eye it felt like a good candidate for the most optimal solution. In the end I think it's interesting enough to keep around and serves as a reminder that simple solutions usually prove superior.

The implementation is internally represented as a graph of nodes, each of which represents an instance of a tag associated with an entry, and stores the tag value as well as double-linked pointers in 2 axes: the entry chain and key chain. The entry list connects nodes which share the same `EntryId` and the key lists (one for each key `K`) connect nodes with the same tag key. These lists then allow fast traversal.

Each node also stores the `EntryId` and a reference to the key to allow fast access during iteration without needing to iterate the respective lists.

Predecessor and end pointers allow inserting to the back of the lists instead of to the front to preserve insertion order, though no concrete guarantees were taken into consideration.

```
                       keys
        ┌───────────────────────────────┐
        │ A      B      C      D      E │
        └─┬──────┬──────┬──────┬──────┬─┘
entries   │      │      │      │      │
 ┌───┐    ▼      ▼      │      ▼      │    ┌───┐
 │ 1 ┼──► A1 ◄─► B1 ◄───┼────► D1 ◄───┼────┼ 1 │
 │   │    ▲      ▲      │      ▲      │    │   │
 │   │    │      │      │      │      │    │   │
 │   │    │      ▼      ▼      │      │    │   │
 │ 2 ┼────┼────► B2 ◄─► C2 ◄───┼──────┼────┼ 2 │
 │   │    │      ▲      ▲      │      │    │   │
 │   │    │      │      │      │      │    │   │
 │   │    ▼      ▼      │      │      │    │   │
 │ 3 ┼──► A3 ◄─► B3 ◄───┼──────┼──────┼────┼ 3 │
 │   │    ▲      ▲      │      │      │    │   │
 │   │    │      │      │      │      │    │   │
 │   │    ▼      ▼      ▼      ▼      ▼    │   │
 │ 4 ┼──► A4 ◄─► B4 ◄─► C4 ◄─► D4 ◄─► E4 ◄─┼ 4 │
 │   │    ▲      ▲      ▲      ▲      ▲    │   │
 │   │    │      │      │      │      │    │   │
 │   │    ▼      │      ▼      │      ▼    │   │
 │ 5 ┼──► A5 ◄───┼────► C5 ◄───┼────► E5 ◄─┼ 5 │
 └───┘    ▲      │      ▲      │      ▲    └───┘
          │      │      │      │      │
        ┌─┼──────┼──────┼──────┼──────┼─┐
        │ A      B      C      D      E │
        └───────────────────────────────┘
```

In the example case above each tag key has a unique key within an entry, but this is not enforced. In the case of duplicate keys, the links between nodes would be "parallel" to the ones in the entry list.

This implementation seemingly has the same time complexities as the [indexed linear](#indexed-linear-implementation). Heap usage is also comparable. My best guess for why it ends up doing worse in time benchmarks is worse cache locality.
