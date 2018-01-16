//! InDirection Optimized Total Order Multimap
//!
//! A multimap which keeps the total order of insertion
//! (don't groups them by key). An still grants resonable
//! fast access to the values.
//!
//! It is implemented as a vector of key, value pairs
//! where the value has to be a container implementing
//! stable deref (e.g. Box<T>) as well as a (multi-)map
//! which contains pointers to the data contained by the
//! values, granting fast access to it.
//!
//!
//! Currently limited to Copy types, could support clone types
//! but would be often quite a bad idea (except for shepish
//! clonable types like e.g. a `Rc`/`Arc`).
// a version of TotalOrderMultiMap could be build wich also relies on inner pointer
// address stability to cheaply share the keys (would also
// work with `&'static str` as `&T` is `StableDeref`).
// The problem is how to handle ownership in that case,
// so not done for now.

extern crate stable_deref_trait;

use std::collections::{ HashMap, hash_map};
use std::ops::Deref;
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


pub trait Meta {
    type MergeError;

    // the "can we update" logic is seperated from the update logic to
    // enforce that no possible updates can never,
    fn check_update(&self, other: &Self) -> Result<(), Self::MergeError>;
    fn update(&mut self, other: Self);

}


// workaround for rust #35121 (`!` type)
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Unreachable { _noop: () }

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash)]
pub struct NoMeta;
impl Meta for NoMeta {
    //TODO(UPSTREAM): experimental/nightly (rustc #35121)
    //type MergeError = !;
    type MergeError = Unreachable;


    fn check_update(&self, _other: &Self) -> Result<(), Self::MergeError> {
        Ok(())
    }
    fn update(&mut self, _other: Self) {}
}

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
// - UNDER ANY CIRCUMSTANC NEVER return a mutable reference to the
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
// - reminder: implementing `StableDeref` for a trait which on a safty level
//   relies on sideffects (e.g. using inner mutability) in deref is unsafe
pub struct TotalOrderMultiMap<K, V, M>
    where V: Deref, K: Hash + Eq + Copy
{
    vec_data: Vec<(K, V)>,
    map_access: HashMap<K, (M, Vec<*const V::Target>)>,
}

impl<K, V, M> Default for TotalOrderMultiMap<K, V, M>
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

impl<K, V, M> TotalOrderMultiMap<K, V, M>
    where K: Hash+Eq+Copy,
          V: StableDeref,
          M: Meta

{
    pub fn new() -> Self {
        Default::default()
    }

    pub fn with_capacity(capacity: usize) -> Self {
        TotalOrderMultiMap {
            vec_data: Vec::with_capacity(capacity),
            map_access: HashMap::with_capacity(capacity),
        }
    }

    pub fn capacity(&self) -> usize {
        self.vec_data.capacity()
    }

    /// # Panics
    /// if the new allocation size overflows `usize`
    pub fn reserve(&mut self, additional: usize) {
        self.vec_data.reserve(additional);
        self.map_access.reserve(additional);
    }

    /// reverses internal (insertion) order
    pub fn reverse(&mut self) {
        self.vec_data.reverse();
        for (_, val) in self.map_access.iter_mut() {
            val.1.reverse()
        }
    }

    pub fn shrink_to_fit(&mut self) {
        self.vec_data.shrink_to_fit();
        self.map_access.shrink_to_fit();
    }

    pub fn len(&self) -> usize {
        self.vec_data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.vec_data.is_empty()
    }

    //
    //drain range? what about drop+recovery safety while drain, map is already cleared...
//    fn drain(&mut self) -> vec::Drain<(K, V)> {
//        self.map_access.clear();
//        self.vec_data.drain(..)
//    }

    pub fn clear(&mut self) {
        self.map_access.clear();
        self.vec_data.clear();
    }

    pub fn contains_key(&self, k: K) -> bool {
        self.map_access.contains_key(&k)
    }

    pub fn get(&self, k: K) -> Option<EntryValues<V::Target, M>>{
        self.map_access.get(&k)
            .map(|vec| EntryValues {
                inner_iter: vec.1.iter(),
                meta: &vec.0,
            } )
    }

//    pub fn get_mut(&mut self, k: K) -> Option<EntryValuesMut<V::Target>> {
//        self.map_access.get_mut(&k)
//            .map(|vec| EntryValuesMut(vec.iter_mut()))
//    }

    /// inserts a key, value pair returns the new count of values for the given key
    pub fn insert(&mut self, key: K, value: V, meta: M) -> Result<usize, (K, V, M, M::MergeError)> {
        use self::hash_map::Entry::*;


        let ptr: *const V::Target = &*value;
        let entry = self.map_access.entry(key);
        let meta_updated_data =
            match entry {
                Occupied(oe) => {
                    let data = oe.into_mut();
                    if let Err(err) = data.0.check_update(&meta) {
                        return Err((key, value, meta, err));
                    }
                    data.0.update(meta);
                    data
                },
                Vacant(ve) => {
                    ve.insert((meta, vec![]))
                }
            };
        //SAFETY: for unwind safety we have to **always** push the box/owning part befor
        // the pointer, that why 1st update meta (it can fail) then update `vec_data`
        // and then update the list we got from the 1st step.
        self.vec_data.push((key,value));
        meta_updated_data.1.push(ptr);
        Ok(meta_updated_data.1.len())
    }

    pub fn pop(&mut self) -> Option<(K, V)> {
        if self.vec_data.is_empty() {
            None
        } else {
            {
                let &(k, ref val) = &self.vec_data[self.vec_data.len()-1];
                let val_ptr: *const V::Target = &**val;
                let vec = &mut self.map_access.get_mut(&k)
                    .expect("[BUG] key in vec_data but not map_access")
                    .1;
                let to_remove = vec.iter().rposition(|ptr| {
                    *ptr == val_ptr
                }).expect("[BUG] no ptr for value in map_access");
                vec.remove(to_remove);
            }
            let res = self.vec_data.pop();
            //unessesary sanity check
            debug_assert!(res.is_some());
            res
        }
    }

    //FIXME(UPSTREAM): use drain_filter instead of retain once stable then return Vec<V>
    // currently it returns true as long as at last one element is removed
    // once `drain_where` (or `drain_filter`) is stable it should be changed
    // to returning the removed values
    pub fn remove_all(&mut self, key_to_remove: K) -> bool {
        if let Some(_) = self.map_access.remove(&key_to_remove) {
            self.vec_data.retain(|&(key, _)| key != key_to_remove);
            true
        } else {
            false
        }
    }

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
                    match values.1.iter().position(|x| *x == ptr) {
                        Some(idx) => {
                            values.1.remove(idx);
                        },
                        None => unreachable!(
                            "[BUG] inconsistent state, value is not in map_access but in vec_data")
                    }
                    //FIXME(TEST): test remove map entry if values.1.is_empty()
                    if !values.1.is_empty() {
                        continue
                    }
                }

            }
            self.map_access.remove(&key);
        }
    }

    ///
    /// # Error
    /// All k-v-pairs which can be added to this map, if one or
    /// more errors occurred `Err(..)` is returned with a vector of
    /// (K, V, Error)-tuples is returned.
    ///
    /// Note that even if `Err(..)` is returned all key-value pairs
    /// which can be added to `self` are added.
    pub fn extend<I>(&mut self, other: I) -> Result<(), Vec<(K, V, M, M::MergeError)>>
        where I: IntoIterator<Item=(K, V, M)>
    {
        let mut errors = Vec::new();
        for (key, val, meta) in other.into_iter() {
            let res = self.insert(key, val, meta);
            if let Err(err) = res {
                errors.push(err)
            }
        }
        if errors.len() == 0 {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl<K, V, M> TotalOrderMultiMap<K, V, M>
    where K: Hash+Eq+Copy,
          V: StableDeref,
          M: Meta + Clone
{
    pub fn into_iter_with_meta(self) -> IntoIterWithMeta<K, V, M> {
        IntoIterWithMeta {
            vec_data: self.vec_data.into_iter(),
            map_access: self.map_access
        }
    }
}

impl<K, V, M> Debug for TotalOrderMultiMap<K, V, M>
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

impl<K, V, M> Clone for TotalOrderMultiMap<K, V, M>
    where K: Hash + Eq + Copy,
          V: StableDeref + Clone,
          M: Meta + Clone
{
    fn clone(&self) -> Self {
        use self::hash_map::Entry::*;

        let mut vec_data = Vec::with_capacity(self.vec_data.len());
        let mut map_access = HashMap::with_capacity(self.map_access.len());

        for &(k, ref val) in self.vec_data.iter() {
            let nval = val.clone();
            let nptr: *const V::Target = &*nval;
            vec_data.push((k, val.clone()));
            match map_access.entry(k) {
                Occupied(mut oe) => {
                    let access: &mut (M, Vec<*const V::Target>) = oe.get_mut();
                    access.1.push(nptr);
                },
                Vacant(ve) => {
                    let new_meta = self.map_access
                        .get(&k)
                        .expect("[BUG] map entries have to exist")
                        .0.clone();
                    ve.insert((new_meta, vec![nptr]));
                }
            }
        }
        TotalOrderMultiMap { map_access, vec_data }
    }
}


impl<K, V, M> PartialEq<Self> for TotalOrderMultiMap<K, V, M>
    where K: Hash + Eq + Copy,
          V: StableDeref + PartialEq<V>,
          M: PartialEq<M>
{
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        //TODO compare meta data
        self.vec_data.eq(&other.vec_data)
    }
}

impl<K, V, M> Eq for TotalOrderMultiMap<K, V, M>
    where K: Hash + Eq + Copy,
          V: StableDeref + Eq,
          M: Eq
{}


impl<K, V, M> IntoIterator for TotalOrderMultiMap<K, V, M>
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

impl<K, V, M> FromIterator<(K, V)> for TotalOrderMultiMap<K, V, M>
    where K: Hash + Eq + Copy,
          V: StableDeref,
          M: Meta<MergeError=Unreachable> + Default
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

//TODO(UPSTREAM): use `!` experimental/nightly (rustc #35121)
impl<K, V, M> Extend<(K, V)> for TotalOrderMultiMap<K, V, M>
    where K: Hash + Eq + Copy,
          V: StableDeref,
          M: Meta<MergeError=Unreachable> + Default,
{
    fn extend<I>(&mut self, src: I)
        where I: IntoIterator<Item=(K,V)>
    {
        for (key, value) in src.into_iter() {
            let e = self.insert(key, value, M::default());
            assert!(e.is_ok(), "Err(Unreachable) can not be created safely, but we still got it");
        }
    }
}

//TODO(UPSTREAM): use `!` experimental/nightly (rustc #35121)
impl<K, V, M> Extend<(K, V, M)> for TotalOrderMultiMap<K, V, M>
    where K: Hash + Eq + Copy,
          V: StableDeref,
          M: Meta<MergeError=Unreachable>
{
    fn extend<I>(&mut self, src: I)
        where I: IntoIterator<Item=(K,V,M)>
    {
        for (key, value, meta) in src.into_iter() {
            let e = self.insert(key, value, meta);
            assert!(e.is_ok(), "Err(Unreachable) can not be created safely, but we still got it");
        }
    }
}



pub struct IntoIterWithMeta<K,V,M>
    where K: Hash+Eq+Copy,
          V: StableDeref,
          M: Meta + Clone
{
    vec_data: vec::IntoIter<(K,V)>,
    map_access: HashMap<K, (M, Vec<*const V::Target>)>
}

impl<K, V, M> Iterator for IntoIterWithMeta<K, V, M>
    where K: Hash+Eq+Copy,
          V: StableDeref,
          M: Meta + Clone
{
    type Item = (K, V, M);

    fn next(&mut self) -> Option<Self::Item> {
        self.vec_data.next()
            .map(|(key, val)| {
                let meta = self.map_access.get(&key)
                    .expect("[BUG] key in `vec_data` but not `map_access`")
                    .0.clone();
                (key, val, meta)
            })
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.vec_data.size_hint()
    }
}

impl<K, V, M> ExactSizeIterator for IntoIterWithMeta<K, V, M>
    where K: Hash + Eq + Copy,
          V: StableDeref,
          M: Meta + Clone
{
    fn len(&self) -> usize {
        self.vec_data.len()
    }
}

impl<K, V, M> Debug for IntoIterWithMeta<K, V, M>
    where K: Hash + Eq + Copy + Debug,
          V: StableDeref + Debug,
          M: Meta + Clone + Debug
{
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        let meta = DebugIterableOpaque::new(
            self.map_access.iter().map(|(&key, &(ref meta, _))| {
                (key, meta)
            })
        );
        fter.debug_struct("IntoIterWithMeta")
            .field("key_value_iter", &self.vec_data)
            .field("meta_data", &meta)
            .finish()
    }
}


pub struct EntryValues<'a, T: ?Sized+'a, M: 'a>{
    inner_iter: slice::Iter<'a, *const T>,
    meta: &'a M
}

impl<'a, T, M> EntryValues<'a, T, M>
    where T: ?Sized + 'a,
          M: 'a
{
    pub fn meta(&self) -> &'a M {
        self.meta
    }
}

//FIMXE(UPSTREAM): StableDerefPlusMut
//pub struct EntryValuesMut<'a, T: ?Sized+'a>(slice::IterMut<'a, *const T>);


//for some reason derive does not work...
impl<'a, T:?Sized+'a, M> Clone for EntryValues<'a, T, M> {
    fn clone(&self) -> Self {
        EntryValues {
            inner_iter: self.inner_iter.clone(),
            meta: self.meta
        }
    }
}

impl<'a, T: ?Sized + 'a, M: 'a> Iterator for EntryValues<'a, T, M> {
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

impl<'a, T: ?Sized + 'a, M: 'a> ExactSizeIterator for EntryValues<'a, T, M> {

    #[inline]
    fn len(&self) -> usize {
        self.inner_iter.len()
    }
}

impl<'a, T, M> Debug for EntryValues<'a, T, M>
    where T: ?Sized + Debug + 'a,
          M: Debug + 'a
{

    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        let metoo = DebugIterableOpaque::new(self.clone());
        fter.debug_struct("EntryValues")
            .field("inner_iter", &metoo)
            .field("meta", self.meta)
            .finish()
    }
}


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


#[cfg(test)]
mod test {
    use std::collections::HashSet;
    use super::*;


    use std::sync::Arc;
    fn arc_str(s: &str) -> Arc<str> {
        <Arc<str> as From<String>>::from(s.to_owned())
    }

    #[test]
    fn aux_fns() {
        let mut map = TotalOrderMultiMap::with_capacity(10);

        assert_eq!(true, map.is_empty());

        assert_ok!(map.insert("key1", Box::new(13u32), NoMeta));

        assert_eq!(false, map.is_empty());
        assert_ok!(map.insert("key2", Box::new(1), NoMeta));

        assert_eq!(10, map.capacity());
        assert_eq!(2, map.len());

        map.shrink_to_fit();

        assert_eq!(2, map.capacity());

        // this is not reserve_exact so it can reserve more
        // than space for one element
        map.reserve(1);

        assert!(map.capacity() >= 3);

        assert_ok!(map.insert("key1", Box::new(44), NoMeta));
        assert_ok!(map.insert("key1", Box::new(44), NoMeta));

        assert_eq!(4, map.len());
        assert!(map.capacity() >= 4);

        map.clear();
        assert_eq!(true, map.is_empty());
        assert_eq!(0, map.len());
    }

    #[test]
    fn works_with_trait_objects() {
        use std::fmt::Debug;
        let mut map = TotalOrderMultiMap::<&'static str, Box<Debug>, NoMeta>::new();
        assert_ok!(map.insert("hy", Box::new("h".to_owned()), NoMeta));
        assert_ok!(map.insert("hy", Box::new(2), NoMeta));
        assert_ok!(map.insert("ho", Box::new("o".to_owned()), NoMeta));

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
        assert_ok!(map.insert("k1", a, NoMeta));
        assert_ok!(map.insert("k1", co_a, NoMeta));
        assert_ok!(map.insert("k1", eq_a.clone(), NoMeta));
        assert_ok!(map.insert("k2", eq_a, NoMeta));
        assert_ok!(map.insert("k3", arc_str("y"), NoMeta));
        assert_ok!(map.insert("k4", arc_str("z"), NoMeta));
        assert_ok!(map.insert("k4", arc_str("a"), NoMeta));
        assert_ok!(map.insert("k1", arc_str("e"), NoMeta));

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
        assert_ok!(map.insert("k1", "hy", NoMeta));
        assert_ok!(map.insert("k2", "ho", NoMeta));
        assert_ok!(map.insert("k1", "last", NoMeta));

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
        assert_ok!(map.insert("k1", "ok", NoMeta));
        assert_ok!(map.insert("k2", "why not?", NoMeta));
        map.reverse();

        assert_eq!(
            [("k2", "why not?"), ("k1", "ok")],
            map.iter().collect::<Vec<_>>().as_slice()
        );
    }


    #[test]
    fn remove_all() {
        let mut map = TotalOrderMultiMap::new();
        assert_ok!(map.insert("k1", "ok", NoMeta));
        assert_ok!(map.insert("k2", "why not?", NoMeta));
        assert_ok!(map.insert("k1", "run", NoMeta));
        assert_ok!(map.insert("k2", "jump", NoMeta));
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
        assert_ok!(map.insert("k1", "ok", NoMeta));
        assert_ok!(map.insert("k2", "why not?", NoMeta));
        assert_ok!(map.insert("k1", "run", NoMeta));
        assert_ok!(map.insert("k2", "uh", NoMeta));

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
        assert_ok!(map.insert("k1", v1.clone(), NoMeta));
        assert_ok!(map.insert("k2", v1.clone(), NoMeta));
        assert_ok!(map.insert("k1", arc_str("v2"), NoMeta));
        assert_ok!(map.insert("k1", v1, NoMeta));

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


}