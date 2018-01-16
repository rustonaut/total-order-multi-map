total-order-multi-map [![Crates.io](https://img.shields.io/crates/v/total-order-multi-map.svg)](https://crates.io/crates/total-order-multi-map) [![total-order-multi-map](https://docs.rs/total-order-multi-map/badge.svg)](https://docs.rs/total-order-multi-map) [![License](https://img.shields.io/badge/License-MIT%2FApache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0) [![Build Status](https://travis-ci.org/1aim/total-order-multi-map.svg?branch=master)](https://travis-ci.org/1aim/total-order-multi-map)
=============

A multimap with at the same time keeps the total insertion ordering of all elements,
this means if you insert following key-value pairs: `(K1, V1)`, `(K2, V2)`, `(K1, V3)`
It will remember that the insertion oder was exact this (`V1`, `V2`, `V3`) i.e.
it won't collaps the insetion order on a per-key basis.

At the same time it should have similar performance for accessing keys as a normal
`HashMap` (else we could have just created a `Vec`).

It does so by limiting its values to such which implement `StableDeref`, e.g.
`Box<TraitObject>` through which it can have more than one ~reference~ pointer to
a value. While it uses unsafety internally the provided interface is safe.

Additionally it allows the storage of one Meta entry per key which can reject
the insertion of a new entry to the given bucket (i.e. under the given key).
While this part of the implementation works quite well it needs to be flushed
out a bit more for a general purpose use case.   

Example
=======

```rust
extern crate total_order_multi_map;


fn main() {
  
}


```

License
=======
Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

Contribution
------------
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.


Change Log
==========
