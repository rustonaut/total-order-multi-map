total-order-multi-map [![Crates.io](https://img.shields.io/crates/v/total-order-multi-map.svg)](https://crates.io/crates/total-order-multi-map) [![total-order-multi-map](https://docs.rs/total-order-multi-map/badge.svg)](https://docs.rs/total-order-multi-map) [![License](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT) [![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
=============

A multi-map with at the same time keeps the total insertion ordering of all elements,
this means if you insert following key-value pairs: `(K1, V1)`, `(K2, V2)`, `(K1, V3)`
It will remember that the insertion oder was exact this (`V1`, `V2`, `V3`) i.e.
it won't collapse the insertion order on a per-key basis.

At the same time it should have similar performance for accessing entries as a normal
`HashMap` (else we could have just created a `Vec`).

It does so by limiting its values to such which implement `StableDeref`, e.g.
`Box<TraitObject>` through which it can have more than one ~reference~ pointer to
a value. While it uses unsafety internally the provided interface is safe.
It also makes sure to be unwind + resume safe.

Example
--------

```rust
extern crate total_order_multi_map as tomm;

use std::fmt::{self, Display};
use tomm::TotalOrderMultiMap;

/// for now the key has to be copy
type Key = &'static str;

/// the map is made for thinks which dereference
/// e.g. trait objects, through e.g. `String` or
/// `Rc<Debug>` would be fine, too.
type Value = Box<Display>;


fn main() {
    let mut map = TotalOrderMultiMap::<Key, Value>::new();

    map.add("key1", mk_box_str("val1"));
    map.add("key1", mk_my_thingy("val2"));
    map.add("key2", mk_box_str("val3"));
    map.add("key1", mk_my_thingy("val4"));
    map.add("key0", mk_box_str("val5"));

    let stringed = map
        .iter()
        // val is of type `&Display`
        .map(|(key, val)| format!("{}:{}", key, val))
        .collect::<Vec<_>>();

    // as it can be seen the total insertion order was kept
    assert_eq!(stringed, &[
        "key1:val1".to_owned(),
        "key1:val2".to_owned(),
        "key2:val3".to_owned(),
        "key1:val4".to_owned(),
        "key0:val5".to_owned()
    ]);

    // It's also still a multi map.
    // Note that the get operation has roughly the same
    // performance as in a hash map (through you then
    // iterate over the elements associated with the key
    // roughly with the speed of iterating over an `Vec`).
    {
        let values = map.get("key1").expect("key1 is in the map");
        let stringed = values
            .map(|val| format!("{}", val))
            .collect::<Vec<_>>();

        assert_eq!(stringed, &[ "val1", "val2", "val4" ]);
    }

    // but as we have an order we can remove
    // "the last inserted" element
    let (key, val) = map.pop().unwrap();
    assert_eq!(format!("{}:{}", key, val), "key0:val5");

    // or remove all for an specific key as it's
    // an multi map
    map.remove_all("key1");
    assert!(map.get("key1").is_none());

    println!("DONE (I only contain asserts)")
}

//---- some function to create dummy values for the example ----

fn mk_box_str(inp: &str) -> Box<Display> {
    // sadly we can't cast Box<str> to Box<Display>
    // (you would need non static vtables for this...)
    Box::new(inp.to_owned())
}

#[derive(Debug)]
struct MyThingy { val: String }

impl Display for MyThingy {
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        write!(fter, "{}", self.val)
    }
}

fn mk_my_thingy(inp: &str) -> Box<Display> {
    Box::new(MyThingy { val: inp.to_owned() })
}
```

Missing Parts
-------------

Following thinks can be implemented and should
be added to further versions:

- benchmarks
- use `SmallVec` for the multi map
- allow `&mut` access to `V::Target` if  `V: StableDeref + DerefMut`
- indexed access
  - indexed get off key-value pairs
  - indexed remove
  - indexed get _in_ the return value of `get`, i.e. a multi map group
- improved entry api
  - allow access to other values for same key
  - potentially turn the result of entry + insert into the result of get + unwrap
- improved grouped iteration, maybe??
  - the current implementation groups by key but doesn't indicate
    when it switches to the next value
- allow non-copy keys??
  - we still don't want to duplicate/clone keys so we would have to do
    some trickery there

License
-------

Licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

Contribution
------------

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.


Change Log
-----------

- `v0.3`:
  - `TotalOrderMultiMap.retain` now accepts a predicate accepting `&V::Target` instead
    of `&V`
- `v0.4.1`:
  - flatten return value to `get` to return empty iterators instead
    of `None`
  - requires `DerefMut` instead of just `Deref`
  - has mutable access methods like `get_mut`
  - split `insert` into `add` and `set` where `set` replaces any value
    associated with the key prev. with the new value returning the old
    value
- `v0.4.2`:
  - mut iter to non mut iter conversions
    - Iter from IterMut
    - Values from ValuesMut
    - EntryValues from EntryValuesMut
- `v0.4.3`:
  - Added missing re-exports. Values, Values,
    GroupedValuesMut are now public as they
    should have been from the beginning.
- `v0.4.4`:
  - Added `truncate` method allowing the removal
    of any think inserted after the first `size`
    elements
- `v0.4.5`:
  - For entries added `values`,`values_mut` methods to
    iterate over the values associated with the key used
    to create the entry.