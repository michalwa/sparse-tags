# sparse-tags

An ECS-like sparse data structure used to store and efficiently iterate over sets of key-value pairs, implemented as a multi-linked list in Rust.

Developed to serve as a search cache for [factbook](https://github.com/michalwa/factbook).

## Architecture

### Definitions

- A key-value pair is called a _tag_. This corresponds to the notion of a _component_ in ECS.
- A group of key-value pairs is called an _entry_, identified by an _entry ID_. This corresponds to an _entity_ in ECS.
- A _store_ holds arbitrarily many entries and their associated tags.

Unlike in traditional ECS architecture, multiple tags within an entry may share the same key. This means an entry is similar to a multi-map or a `Map<K, Vec<V>>` where `K` is the key type and `V` is the value type, moreso than a `Map<K, V>` (specific map implementation left implicit).

Tag keys and values are currently modelled to be homogenous, so implementing an actual ECS using this crate will not provide adequate compile-time guarantees. Though a future extension to support a functionality similar to [`typemap`](https://github.com/reem/rust-typemap) is possible.

The word _sparse_ is used to mean that not all entries necessarily contain tags of all unique keys, and also that the implementation resembles that of a [multi-linked list sparse matrix](https://webdocs.cs.ualberta.ca/~holte/T26/mlinked-lists.html).

### Implementation

The main implementation [`MultiLinkedStore`](src/multi_linked.rs) is internally represented as a graph of nodes, each of which represents an instance of a tag associated with an entry, and stores the tag value as well as double-linked pointers in 2 axes: the entry chain and key chain. The entry list connects nodes which share the same `EntryId` and the key lists (one for each key `K`) connect nodes with the same tag key. These lists then allow fast traversal.

Each node also stores the `EntryId` and a reference to the key to allow fast access during iteration without needing to iterate the respective lists.

Predecessor and end pointers allow inserting to the back of the lists instead of to the front to roughly preserve insertion order, though no guarantees about this are made and it is subject to change in future versions.

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

_A representation of a `MultiLinkedStore` populated with example data. In this case each tag key has a unique key within an entry, but this is not enforced. In the case of duplicate keys, the links between nodes would be "parallel" to the ones in the entry list._

This achieves the following:

- `insert_entry` and `insert_tag` are `O(1)`.
- Iterating `tags_by_entry` and `tags_by_key` is linear in the number of entries or tags, respectively, matching the predicate. No unnecessary nodes are traversed during a search.

The downside is that `remove_entry` and `clear_entry` are linear in the number of tags present on the entry, because all neighboring tag nodes need to be re-linked. Based on the intended use case it is assumed that realistically there will be significantly more entries than keys, and that this is therefore the right tradeoff.
