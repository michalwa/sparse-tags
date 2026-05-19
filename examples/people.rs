use sparse_tags::{MultiLinkedStore, Store};

fn main() {
    let mut store = MultiLinkedStore::new();

    let john = store.insert_entry_with(
        1,
        [
            ("first_name", "John"),
            ("last_name", "Doe"),
            ("city", "Berlin"),
        ],
    );
    let anna = store.insert_entry_with(2, [("first_name", "Anna"), ("city", "Warsaw")]);
    let greg = store.insert_entry_with(3, [("first_name", "Greg"), ("last_name", "Fletcher")]);
    let king = store.insert_entry_with(4, [("last_name", "King")]);

    // List all tags
    // You could also iterate over all entries using `store.entries()`
    for person in [john, greg, anna, king].into_iter() {
        let id = store.entry_data(person);
        println!("Person #{id}: ");

        for (k, v) in store.tags_by_entry(person) {
            println!("  {k}: {v}");
        }
    }

    store.remove_entry(greg);

    // List all known first names and optionally associated last names
    for (person, first_name) in store.tags_by_key(&"first_name") {
        let last_name_tag = store
            .tags_by_entry(person)
            .find(|&(&k, _)| k == "last_name");

        if let Some((_, &last_name)) = last_name_tag {
            println!("{first_name} {last_name}");
        } else {
            println!("{first_name} (no known last name)");
        }
    }
}
