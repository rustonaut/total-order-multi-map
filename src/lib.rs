//! A multi map implementation which also keeps the
//! total order of inserted elements. I.e. if you
//! insert `(k1, v1)` then `(k2, v2)` then `(k1, v3)`.
//! The order when iterating over it will be exact this
//! insertion order. Through there is an `grouped_values`
//! method returning a iterator over the values grouped
//! by key, instead of iteration order.
//!
//! The map makes sure that normal iteration is roughly
//! as fast a iterating over a vector but using `get` to
//! get a (group of) values is also roughly as fast as
//! using `get` on a `HashMap`. The draw back is that
//! insertion is a bit slower.
//!
//! Note that this implementation is made for values
//! which dereference to the actually relevant values,
//! (e.g. `Box<T>`, `Rc<T>`, `Arc<T>`, `&T`) and when
//! accessing the map references to the inner values are
//! returned (e.g. with a `Box<T>` references to `&T` are
//! returned and the `Box` is not accessible).
//!
//! Because of implementation details it is required that
//! the value containers implement `StableDeref`. Note that
//! this multi map can, or more precisely is made to, handle
//! unsized values like trait object or slices.
//!
//! # State of Implementation
//!
//! Currently a lot of possible and useful methods are
//! missing, take a look at the readme for more details.
//! Through core methods like `insert`, `get` and multiple
//! iterators are implemented.
//!
//! # Example
//!
//! see the example directories `from_readme.rs` example,
//! which is also present in the README.
//!
//! # Implementation Details
//!
//! This implementation internally has a `Vec` and
//! a `HashMap`. The `Vec` contains key-value pairs
//! and "owns" the values. It is used for simple
//! iteration and keeps the insertion order. The
//! `HashMap` is a map from keys to vectors of
//! pointers to inner values. And represents
//! the multi map part. The code makes sure that
//! only pointers are in the hash map iff their
//! "owned" value is in the `Vec` at _any_ part
//! of execution, even during insertion. This is
//! needed to make sure that this type is unwind
//! safe. Also to make sure that the pointers to
//! the inner values are always valid the values
//! have to implement `StableDeref`, so even if
//! the vec is moved/resized the pointers stay
//! valid as they don't point into the vec, but
//! to the inner value the value in the vec points
//! to. In turn this also means that mutable references
//! to the containers/values should _never_ be exposed,
//! through mutable references to the inner values
//! can be exposed.
//!
//! Note that for ease of implementation only Copy
//! keys are allowed, this should be improved on in
//! later versions.
//!

extern crate stable_deref_trait;

use std::collections::{ HashMap, hash_map};
use std::{vec, slice};
use std::hash::Hash;
use std::cmp::{Eq, PartialEq};
use std::iter::{ Extend, FromIterator };
use std::fmt::{self, Debug};

use stable_deref_trait::StableDeref;

use self::utils::DebugIterableOpaque;
pub use self::iter::{ Iter, IterMut };
pub use self::entry::Entry;
pub use self::map_iter::{ Keys, GroupedValues };

#[macro_use]
mod utils;
mod iter;
mod entry;
mod map_iter;


// # SAFETY constraints (internal):
//
// - the ptr. contained in map_access have to be always valid,
//   code adding elements should first add them to `vec_data`,
//   and then to `map_access` code removing elements should
//   first remove them from `map_access` and then from `vec_data`.
//
// - giving out references to data always requires `self` to be
//   borrowed by the same kind of reference, a function giving
//   out references (either direct or transitive) should either
//   only use `vec_data` or `map_access` but not both, especially
//   so wrt. `&mut`.
//
// - UNDER ANY CIRCUMSTANCE NEVER return a mutable reference to the
//   data container (`&mut V`) in difference to a `&mut T`/`&mut V::Target`
//   it can override the container invalidating the `StableDeref` assumptions.
//
// ## StableDeref assumptions
//
// - reminder: containers implementing `StableDeref` deref always to the same
//   memory address as long as the container itself is not mutated (but
//   even if the data on the address is mutated)
//
// - reminder: as we keep pointers directly to the data we can't allow any
//   mutation of the container
//
// - reminder: implementing `StableDeref` for a trait which on a safety level
//   relies on side-effects (e.g. using inner mutability) in deref is unsafe
//
/// A multi map with keeps the total ordering of inserted elements
///
/// The map is meant to contain values implementing `StableDeref`,
/// methods like `get` and `iter` will iterate over the inner values
/// referred to when dereferencing the values.
///
/// The key is currently limited to values implementing Copy.
///
/// See the module/crate level documentation for more details.
///
/// # Unwind Safety
///
/// This type is unwind + resume safe.
/// Through in the unlikely case that a panic happens inside of
/// a function like `insert`,`get` the resulting state might be
/// inconsistent in a safe way, i.e. some values might be in the map,
/// accessible during iteration but hey won't appear when using `get`.
///
/// Note that this can only happen in a few rare cases:
///
/// 1. `Vec::pop`/`Vec::reserve` panics
/// 2. `HashMap::insert`/`HashMap::reserve` panics
/// 3. for some reason `oom` does panic instead of aborting
///
/// Which mainly can happen in mainly following cases:
///
/// - the vector of key-value pairs tries to contain more then `isize::MAX` bytes
/// - you use zero-sized values and overflow `usize` (which can't really happen as
///   we store at last some data per inserting, i.e. you would overflow the vec first)
///
/// Generally speaking you won't run into any of this in normal circumstances and if
/// you do it's likely that you are close to a system wide `oom` anyway.
pub struct TotalOrderMultiMap<K, V>
    where V: StableDeref, K: Hash + Eq + Copy
{
    vec_data: Vec<(K, V)>,
    map_access: HashMap<K, Vec<*const V::Target>>,
}

impl<K, V> Default for TotalOrderMultiMap<K, V>
    where K: Hash + Eq + Copy,
          V: StableDeref
{
    fn default() -> Self {
        TotalOrderMultiMap {
            vec_data: Default::default(),
            map_access: Default::default()
        }
    }

}

// Note further implementations in sub-modules (entry.rs, iter.rs, ...)
impl<K, V> TotalOrderMultiMap<K, V>
    where K: Hash+Eq+Copy,
          V: StableDeref
{

    /// create a new empty map
    pub fn new() -> Self {
        Default::default()
    }

    /// crate a new map with a given `capacity`
    ///
    /// Note that this method will reserve the given
    /// capacity for the internal `Vec` and `HashMap`,
    /// but as it's a multi map it can not know how
    /// elements will be distributed, as such using
    /// `with_capacity` does _not_ guarantee that there
    /// are no allocations if less than `capacity` elements
    /// are inserted. Through it still can reduce the
    /// number of allocations needed.
    pub fn with_capacity(capacity: usize) -> Self {
        TotalOrderMultiMap {
            vec_data: Vec::with_capacity(capacity),
            map_access: HashMap::with_capacity(capacity),
        }
    }

    /// returns the capacity (unreliable)
    ///
    /// This is not reliable as it only returns
    /// the capacity of the underlying `Vec` not
    /// considering the underlying `HashMap` or
    /// the `Vec` used to turn the map into a
    /// multi map.
    pub fn capacity(&self) -> usize {
        self.vec_data.capacity()
    }

    /// reserves space for `n` additional elements
    ///
    /// The reservation is done in both the internal
    /// `Vec` and `HashMap` but as the map is a multi
    /// map this method is less helpful as e.g. on a
    /// pure `Vec` or `HashMap`
    ///
    /// # Panics
    /// if the new allocation size overflows `usize`
    pub fn reserve(&mut self, additional: usize) {
        self.vec_data.reserve(additional);
        self.map_access.reserve(additional);
    }

    /// reverses insertion order
    ///
    /// After calling this the map will contains values
    /// as if they had been inserted in reversed order.
    ///
    /// This will affect both the iteration order of
    /// the fill map as well as the iteration order of
    /// values returned by `get`.
    pub fn reverse(&mut self) {
        self.vec_data.reverse();
        for (_, val) in self.map_access.iter_mut() {
            val.reverse()
        }
    }

    /// shrinks all internal containers to not contains any additional capacity
    ///
    /// Whether or not memory is freed depends in the dens on the `shrink_to_fit`
    /// implementation of `Vec` and `HashMap`
    pub fn shrink_to_fit(&mut self) {
        self.vec_data.shrink_to_fit();
        self.map_access.shrink_to_fit();
        for (_, val) in self.map_access.iter_mut() {
            val.shrink_to_fit()
        }
    }

    /// returns the total number of elements
    pub fn len(&self) -> usize {
        self.vec_data.len()
    }

    /// returns the total number of different keys
    pub fn key_count(&self) -> usize {
        self.map_access.len()
    }

    /// true if the map is empty
    pub fn is_empty(&self) -> bool {
        self.vec_data.is_empty()
    }

    /// empties this map
    pub fn clear(&mut self) {
        self.map_access.clear();
        self.vec_data.clear();
    }

    /// true if the key is contained in the map
    ///
    /// does not state how many values are associated with it
    pub fn contains_key(&self, k: K) -> bool {
        self.map_access.contains_key(&k)
    }

    /// returns values associated with the given key
    ///
    /// If the key is not in the map this will return `None`.
    /// This also means that `EntryValues` has at at alst one
    /// element.
    pub fn get(&self, k: K) -> Option<EntryValues<V::Target>>{
        self.map_access.get(&k)
            .map(|vec| EntryValues {
                inner_iter: vec.iter(),
            } )
    }

//    pub fn get_mut(&mut self, k: K) -> Option<EntryValuesMut<V::Target>> {
//        self.map_access.get_mut(&k)
//            .map(|vec| EntryValuesMut(vec.iter_mut()))
//    }

    /// inserts a key, value pair and returns the new count of values for the given key
    pub fn insert(&mut self, key: K, value: V) -> usize {
        use self::hash_map::Entry::*;

        let ptr: *const V::Target = &*value;
        self.vec_data.push((key, value));
        match self.map_access.entry(key) {
            Occupied(oe) => {
                let data = oe.into_mut();
                data.push(ptr);
                data.len()
            },
            Vacant(ve) => {
                ve.insert(vec![ptr]).len()
            }
        }
    }

    /// remove and return the element last inserted
    pub fn pop(&mut self) -> Option<(K, V)> {
        if self.vec_data.is_empty() {
            None
        } else {
            {
                let &(k, ref val) = &self.vec_data[self.vec_data.len() - 1];
                let val_ptr: *const V::Target = &**val;
                let vec = &mut self.map_access.get_mut(&k)
                    .expect("[BUG] key in vec_data but not map_access");

                let to_remove = vec.iter().rposition(|ptr| {
                    *ptr == val_ptr
                }).expect("[BUG] no ptr for value in map_access");
                vec.remove(to_remove);
            }
            let res = self.vec_data.pop();
            debug_assert!(res.is_some(), "sanity check that we indeet did pop something");
            res
        }
    }

    //FIXME(UPSTREAM): use drain_filter instead of retain once stable then return Vec<V>
    // currently it returns true as long as at last one element is removed
    // once `drain_where` (or `drain_filter`) is stable it should be changed
    // to returning the removed values
    /// removes all values associated with the given key
    ///
    /// I.e. this removes all key-value pairs which key
    /// is equal to the given key.
    ///
    /// Returns true if at last one value was removed
    pub fn remove_all(&mut self, key_to_remove: K) -> bool {
        if let Some(_) = self.map_access.remove(&key_to_remove) {
            self.vec_data.retain(|&(key, _)| key != key_to_remove);
            true
        } else {
            false
        }
    }

    //TODO make sure this plays nicely with panic resume
    /// retains only key value pairs for which `func` returns true
    ///
    /// All key-value pairs for with the predicate `func` returns
    /// false will be removed.
    pub fn retain<FN>(&mut self, mut func: FN)
        where FN: FnMut(K, &V) -> bool
    {
        let mut to_remove = Vec::new();
        self.vec_data.retain(|&(key, ref v)| {
            let retrain = func(key, v);
            if !retrain {
                let vptr: *const V::Target = &**v;
                to_remove.push((key, vptr));
            }
            retrain
        });
        for (key, ptr) in to_remove.into_iter() {
            {
                if let Some(values) = self.map_access.get_mut(&key) {
                    //TODO use remove_item once stable (rustc #40062) [inlined unstable def]
                    match values.iter().position(|x| *x == ptr) {
                        Some(idx) => {
                            values.remove(idx);
                        },
                        None => unreachable!(
                            "[BUG] inconsistent state, value is not in map_access but in vec_data")
                    }
                    //FIXME(TEST): test remove map entry if values.is_empty()
                    if !values.is_empty() {
                        continue
                    }
                }

            }
            self.map_access.remove(&key);
        }
    }
}

impl<K, V> Debug for TotalOrderMultiMap<K, V>
    where K: Hash + Eq + Copy + Debug,
          V: StableDeref + Debug
{
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        write!(fter, "TotalOrderMultiMap {{ ")?;
        for &(key, ref val_cont) in self.vec_data.iter() {
            write!(fter, "{:?} => {:?},", key, val_cont)?;
        }
        write!(fter, " }}")
    }
}

impl<K, V> Clone for TotalOrderMultiMap<K, V>
    where K: Hash + Eq + Copy,
          V: StableDeref + Clone
{
    fn clone(&self) -> Self {
        let vec_data = Vec::with_capacity(self.vec_data.len());
        let map_access = HashMap::with_capacity(self.map_access.len());
        let mut map = TotalOrderMultiMap { map_access, vec_data};

        for &(k, ref val) in self.vec_data.iter() {
            map.insert(k, val.clone());
        }

        map
    }
}

/// Compares for equality which does consider the insertion order
impl<K, V> PartialEq<Self> for TotalOrderMultiMap<K, V>
    where K: Hash + Eq + Copy,
          V: StableDeref + PartialEq<V>,
{
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.vec_data.eq(&other.vec_data)
    }
}

/// Compares for equality which does consider the insertion order
impl<K, V> Eq for TotalOrderMultiMap<K, V>
    where K: Hash + Eq + Copy,
          V: StableDeref + Eq,
{}


impl<K, V> IntoIterator for TotalOrderMultiMap<K, V>
    where K: Hash + Eq + Copy,
          V: StableDeref,
{
    type Item = (K, V);
    type IntoIter = vec::IntoIter<(K, V)>;


    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        let TotalOrderMultiMap { vec_data, map_access } = self;
        drop(map_access);
        vec_data.into_iter()
    }
}

impl<K, V> FromIterator<(K, V)> for TotalOrderMultiMap<K, V>
    where K: Hash + Eq + Copy,
          V: StableDeref,
{

    fn from_iter<I: IntoIterator<Item=(K,V)>>(src: I) -> Self {
        let src_iter = src.into_iter();

        let mut out = {
            let (min, _) = src_iter.size_hint();
            if min > 0 {
                Self::with_capacity(min)
            } else {
                Self::default()
            }
        };
        <Self as Extend<(K,V)>>::extend(&mut out, src_iter);
        out
    }
}

impl<K, V> Extend<(K, V)> for TotalOrderMultiMap<K, V>
    where K: Hash + Eq + Copy,
          V: StableDeref
{
    fn extend<I>(&mut self, src: I)
        where I: IntoIterator<Item=(K,V)>
    {
        for (key, value) in src.into_iter() {
            self.insert(key, value);
        }
    }
}

/// A type providing access to all values associated to "some key"
///
/// This is mainly an iterator over values, or more precisely
/// references to the inner value of the values.
///
/// This is returned by `TotalOrderMultiMap.get`, so it does not
/// contain the key as it should be known in any context where this
/// type appear.
///
pub struct EntryValues<'a, T: ?Sized+'a>{
    inner_iter: slice::Iter<'a, *const T>,
}

impl<'a, T> EntryValues<'a, T>
    where T: ?Sized + 'a
{}


// This iterator can be cloned cheaply
impl<'a, T: ?Sized + 'a> Clone for EntryValues<'a, T> {
    fn clone(&self) -> Self {
        EntryValues {
            inner_iter: self.inner_iter.clone(),
        }
    }
}

impl<'a, T: ?Sized + 'a> Iterator for EntryValues<'a, T> {
    type Item = &'a T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        //SAFE: the pointers are guaranteed to be valid, at last for lifetime 'a
        self.inner_iter.next().map(|&ptr| unsafe { &*ptr })
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner_iter.size_hint()
    }
}

impl<'a, T: ?Sized + 'a> ExactSizeIterator for EntryValues<'a, T> {

    #[inline]
    fn len(&self) -> usize {
        self.inner_iter.len()
    }
}

impl<'a, T> Debug for EntryValues<'a, T>
    where T: ?Sized + Debug + 'a
{

    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        let metoo = DebugIterableOpaque::new(self.clone());
        fter.debug_struct("EntryValues")
            .field("inner_iter", &metoo)
            .finish()
    }
}

//TODO this was blocked but should now be doable
//pub struct EntryValuesMut<'a, T: ?Sized+'a>(slice::IterMut<'a, *const T>);

//impl<'a, T: ?Sized + 'a> Iterator for EntryValuesMut<'a, T> {
//    type Item = &'a mut T;
//
//    #[inline]
//    fn next(&mut self) -> Option<Self::Item> {
//        //SAFE: the pointers are guaranteed to be valid, at last for lifetime 'a
//        self.0.next().map(|&mut ptr| unsafe { &mut *ptr} )
//    }
//
//    #[inline]
//    fn size_hint(&self) -> (usize, Option<usize>) {
//        self.0.size_hint()
//    }
//}
//
//impl<'a, T: ?Sized + 'a> ExactSizeIterator for EntryValuesMut<'a, T> {
//
//    #[inline]
//    fn len(&self) -> usize {
//        self.0.len()
//    }
//}
//
//impl<'a, T: ?Sized + 'a> Debug for EntryValuesMut<'a, T> {
//
//    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
//        self.0.fmt(fter)
//    }
//}

//SAFE: only the *const V::Target is not default Send/Sync as it's a pointer but we
//  can say it's safe, as we use the pointer "just" as a alternate way to access data
//  we have a "safe" pointer to (e.g. Box if V is Box) so "from the outside" we can
//  treat it just like that
//
//TODO this is more complex tripple check this
// # Note
//
// Due to the constraints `V: Send + StableDeref` it should be enough if
// `V` is `Send`, i.e. we do not need a constraint on `V::Target`. Practically
// `V::Target` has to be `Sync` or `V` could not have implemented `Send` +
// `StableDeref`
// `V::Target`, doesn't need to be `Send` as long as `V` is send, e.g. `V` could be
// some kind of reference to a value in another thread. For `V` to impl
// `StableDeref + Send` `V::Target` has to be _at last_ `Sync` or `V` could not
// have been `Send`. As such we add this constraint just to be sure.
//
unsafe impl<K: Send, V: Send> Send for TotalOrderMultiMap<K, V>
    where V: StableDeref, K: Hash + Eq + Copy, V::Target: Sync {}

//SAFE: see description for the Send impl
unsafe impl<K: Sync, V: Sync> Sync for TotalOrderMultiMap<K, V>
    where V: StableDeref, K: Hash + Eq + Copy, V::Target: Sync {}


#[cfg(test)]
mod test {
    use std::mem;
    use std::collections::HashSet;
    use super::*;


    use std::sync::Arc;
    fn arc_str(s: &str) -> Arc<str> {
        <Arc<str> as From<String>>::from(s.to_owned())
    }


    #[test]
    fn clone_works_fine() {
        let obj_single = Box::new("hy".to_owned());
        let obj_multi_a = Box::new("there".to_owned());
        let obj_multi_b = Box::new("so".to_owned());

        // make sure the pointer are to the new addresses
        let mut used_addresses = HashSet::new();
        used_addresses.insert(&*obj_single as *const String as usize);
        used_addresses.insert(&*obj_multi_a as *const String as usize);
        used_addresses.insert(&*obj_multi_b as *const String as usize);

        let mut map = TotalOrderMultiMap::with_capacity(10);
        map.insert("k1", obj_single);
        map.insert("k2", obj_multi_a);
        map.insert("k2", obj_multi_b);

        let map2 = map.clone();
        // "hide" map to make sure there
        // is no accidental missuse
        let __map = map;


        //check if the addresses are "new" addresses
        for val in map2.get("k1").unwrap() {
            let ptr = val as *const String as usize;
            assert!(!used_addresses.contains(&ptr));
        }
        for val in map2.get("k2").unwrap() {
            let ptr = val as *const String as usize;
            assert!(!used_addresses.contains(&ptr));
        }

        for (_k, v) in map2.iter() {
            let ptr = v as *const String as usize;
            assert!(!used_addresses.contains(&ptr));
            // here we also make sure there are no collisions
            used_addresses.insert(ptr);
        }

        assert_eq!(used_addresses.len(), 2 * map2.len());

        mem::drop(__map);
        mem::drop(map2);
    }

    #[test]
    fn aux_fns() {
        let mut map = TotalOrderMultiMap::with_capacity(10);

        assert_eq!(true, map.is_empty());

        map.insert("key1", Box::new(13u32));

        assert_eq!(false, map.is_empty());
        map.insert("key2", Box::new(1));

        assert_eq!(10, map.capacity());
        assert_eq!(2, map.len());

        map.shrink_to_fit();

        assert_eq!(2, map.capacity());

        // this is not reserve_exact so it can reserve more
        // than space for one element
        map.reserve(1);

        assert!(map.capacity() >= 3);

        map.insert("key1", Box::new(44));
        map.insert("key1", Box::new(44));

        assert_eq!(4, map.len());
        assert!(map.capacity() >= 4);

        map.clear();
        assert_eq!(true, map.is_empty());
        assert_eq!(0, map.len());
    }

    #[test]
    fn works_with_trait_objects() {
        use std::fmt::Debug;
        let mut map = TotalOrderMultiMap::<&'static str, Box<Debug>>::new();
        map.insert("hy", Box::new("h".to_owned()));
        map.insert("hy", Box::new(2));
        map.insert("ho", Box::new("o".to_owned()));

        let view = map.values().collect::<Vec<_>>();
        assert_eq!(
            "\"h\" 2 \"o\"",
            format!("{:?} {:?} {:?}", view[0], view[1], view[2])
        )
    }

    #[test]
    fn get_set() {
        let mut map = TotalOrderMultiMap::new();
        let a = arc_str("a");
        let co_a = a.clone();
        let eq_a = arc_str("a");
        map.insert("k1", a);
        map.insert("k1", co_a);
        map.insert("k1", eq_a.clone());
        map.insert("k2", eq_a);
        map.insert("k3", arc_str("y"));
        map.insert("k4", arc_str("z"));
        map.insert("k4", arc_str("a"));
        map.insert("k1", arc_str("e"));

        let val_k1 = map.get("k1");
        assert_eq!(true, val_k1.is_some());
        let val_k1 = val_k1.unwrap();
        assert_eq!(4, val_k1.len());
        assert_eq!((4,Some(4)), val_k1.size_hint());
        assert_eq!(["a", "a", "a", "e"], val_k1.collect::<Vec<_>>().as_slice());

        let val_k2 = map.get("k2");
        assert_eq!(true, val_k2.is_some());
        let val_k2 = val_k2.unwrap();
        assert_eq!(1, val_k2.len());
        assert_eq!((1,Some(1)), val_k2.size_hint());
        assert_eq!(["a"], val_k2.collect::<Vec<_>>().as_slice());

        assert_eq!(
            ["a", "a", "a", "a", "y", "z", "a", "e" ],
            map.values().collect::<Vec<_>>().as_slice()
        );

        let mut expected = HashSet::new();
        expected.insert(("k1", vec![ "a", "a", "a", "e" ]));
        expected.insert(("k2", vec![ "a" ]));
        expected.insert(("k3", vec![ "y" ]));
        expected.insert(("k4", vec![ "z", "a" ]));
        assert_eq!(
            expected,
            map.group_iter()
                .map(|giter| (giter.key(), giter.collect::<Vec<_>>()) )
                .collect::<HashSet<_>>()
        );

        assert_eq!(
            [ ("k1", "a"), ("k1", "a"), ("k1", "a"), ("k2", "a"),
                ("k3", "y"), ("k4", "z"), ("k4", "a"), ("k1", "e")],
            map.iter().collect::<Vec<_>>().as_slice()
        );
    }

    #[test]
    fn pop() {
        let mut map = TotalOrderMultiMap::new();
        map.insert("k1", "hy");
        map.insert("k2", "ho");
        map.insert("k1", "last");

        let last = map.pop();
        assert_eq!(
            Some(("k1", "last")),
            last
        );
    }

//    #[test]
//    fn get_mut() {
//        use std::sync::Arc;
//        use std::cell::RefCell;
//        let mut map = TotalOrderMultiMap::new();
//        map.insert("k1", Arc::new(RefCell::new(12)));
//
//        let cell = map.get_mut("k1").unwrap().next().unwrap();
//        *cell.borrow_mut() = 55;
//
//        let exp_cell = RefCell::new(55);
//        assert_eq!(
//            [ ("k1", &exp_cell) ],
//            map.iter().collect::<Vec<_>>().as_slice()
//        )
//    }

    #[test]
    fn reverse() {
        let mut map = TotalOrderMultiMap::new();
        map.insert("k1", "ok");
        map.insert("k2", "why not?");
        map.reverse();

        assert_eq!(
            [("k2", "why not?"), ("k1", "ok")],
            map.iter().collect::<Vec<_>>().as_slice()
        );
    }


    #[test]
    fn remove_all() {
        let mut map = TotalOrderMultiMap::new();
        map.insert("k1", "ok");
        map.insert("k2", "why not?");
        map.insert("k1", "run");
        map.insert("k2", "jump");
        let did_rm = map.remove_all("k1");
        assert_eq!(true, did_rm);
        assert_eq!(false, map.remove_all("not_a_key"));

        assert_eq!(
            [("k2", "why not?"), ("k2", "jump")],
            map.iter().collect::<Vec<_>>().as_slice()
        );
    }

    #[test]
    fn retain() {
        let mut map = TotalOrderMultiMap::new();
        map.insert("k1", "ok");
        map.insert("k2", "why not?");
        map.insert("k1", "run");
        map.insert("k2", "uh");

        map.retain(|key, val| {
            assert!(key.len() == 2);
            val.len() > 2
        });

        assert_eq!(
            [("k2", "why not?"), ("k1", "run")],
            map.iter().collect::<Vec<_>>().as_slice()
        );
    }

    #[test]
    fn retain_with_equal_pointers() {
        let mut map = TotalOrderMultiMap::new();
        let v1 = arc_str("v1");
        map.insert("k1", v1.clone());
        map.insert("k2", v1.clone());
        map.insert("k1", arc_str("v2"));
        map.insert("k1", v1);

        let mut rem_count = 0;
        map.retain(|_key, val| {
            if rem_count >= 2 { return true; }
            if &**val == "v1" {
                rem_count += 1;
                false
            } else {
                true
            }
        });

        assert_eq!(
            [("k1", "v2"), ("k1", "v1")],
            map.iter().collect::<Vec<_>>().as_slice()
        );
    }


    trait AssertSend: Send {}
    impl<K: Send, V: Send> AssertSend for TotalOrderMultiMap<K, V>
        where V: StableDeref, K: Hash + Eq + Copy, V::Target: Sync {}

    trait AssertSync: Sync {}
    impl<K: Sync, V: Sync> AssertSync for TotalOrderMultiMap<K, V>
        where V: StableDeref, K: Hash + Eq + Copy, V::Target: Sync {}
}