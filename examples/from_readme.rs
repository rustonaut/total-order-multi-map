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
        let values = map.get("key1");
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
    assert_eq!(0, map.get("key1").len(), "expected no values for key1");

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