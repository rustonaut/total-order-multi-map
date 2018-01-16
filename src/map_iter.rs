use std::collections::hash_map;
use std::slice;
use std::hash::Hash;
use std::cmp::Eq;
use std::ops::DerefMut;
use std::fmt::{self, Debug};
use std::iter::{Iterator, ExactSizeIterator};

use stable_deref_trait::StableDeref;

use utils::DebugIterableOpaque;

use super::TotalOrderMultiMap;


impl<K, V, M> TotalOrderMultiMap<K, V, M>
    where K: Hash+Eq+Copy,
          V: StableDeref
{
    pub fn keys(&self) -> Keys<K, V::Target, M> {
        Keys(self.map_access.keys())
    }

    /// Returns a iterator over all values grouped by key
    pub fn group_iter(&self) -> GroupedValues<K, V::Target, M> {
        GroupedValues(self.map_access.iter())
    }

    pub fn values(&self) -> Values<K, V> {
        Values(self.vec_data.iter())
    }

    pub fn values_mut(&mut self) -> ValuesMut<K, V> {
        ValuesMut(self.vec_data.iter_mut())
    }

    //UPSTREAM: requires a StableDerefMut
//    pub fn grouped_values_mut(&self) -> GroupedValues<K, V> {
//
//    }
}

#[derive(Clone)]
pub struct Keys<'a, K: 'a, T: ?Sized + 'a, M: 'a>(hash_map::Keys<'a, K, (M, Vec<*const T>)>);

impl<'a, K: Debug+'a, T: 'a, M: 'a> Debug for Keys<'a, K, T, M> {
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(fter)
    }
}

impl<'a, K: 'a, T: 'a, M: 'a> Iterator for Keys<'a, K, T, M>
    where K: Copy
{
    type Item = K;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|&k|k)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<'a, K, T, M> ExactSizeIterator for Keys<'a, K, T, M>
    where K: Copy + 'a, T: 'a
{
    #[inline]
    fn len(&self) -> usize {
        self.0.len()
    }
}

/// a iterator over `&V::Target` values returned by `GroupedValues`
/// It will iterate over all values accociated with a specific key
/// in the order they where inserted.
pub struct Group<'a, K, T, M>
    where K: Copy + 'a,
          T: ?Sized + 'a,
          M: 'a
{
    inner_iter: slice::Iter<'a, *const T>,
    meta: &'a M,
    key: K
}

impl<'a, K, T, M> Clone for Group<'a, K, T, M>
    where K: Copy + 'a,
          T: ?Sized + 'a,
          M: 'a
{
    fn clone(&self) -> Self {
        Group {
            key: self.key.clone(),
            meta: self.meta,
            inner_iter: self.inner_iter.clone(),
        }
    }
}

impl<'a, K, T, M> Group<'a, K, T, M>
    where K: Copy + 'a,
          T: ?Sized + 'a,
          M: 'a
{
    pub fn key(&self) -> K {
        self.key
    }

    pub fn meta(&self) -> &'a M {
        self.meta
    }
}

impl<'a, K, T, M> Iterator for Group<'a, K, T, M>
    where K: Copy + 'a,
          T: ?Sized + 'a,
          M: 'a
{
    type Item = &'a T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        //SAFE see TotalOrderMultiMap safety guarantees/constraints
        self.inner_iter.next().map(|&ptr| unsafe { &*ptr } )
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner_iter.size_hint()
    }
}

impl<'a, K, T, M> ExactSizeIterator for Group<'a, K, T, M>
    where K: Copy + 'a,
          T: ?Sized + 'a,
          M: 'a
{
    #[inline]
    fn len(&self) -> usize {
        self.inner_iter.len()
    }
}

impl<'a, K, T, M> Debug for Group<'a, K, T, M>
    where K: Debug + Copy + 'a,
          T: Debug + ?Sized + 'a,
          M: Debug + 'a
{
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        let values = DebugIterableOpaque::new(self.clone());
        fter.debug_struct("Group")
            .field("key", &self.key())
            .field("meta", self.meta())
            .field("values", &values)
            .finish()
    }
}


/// an iterator of Groups (no fixed iteration order)
pub struct GroupedValues<
    'a,
    K: Copy + 'a,
    T: ?Sized + 'a,
    M: 'a
>(hash_map::Iter<'a, K, (M, Vec<*const T>)>);

impl<'a, K, T, M> Clone for GroupedValues<'a, K, T, M>
    where K: Copy + 'a,
          T: ?Sized + 'a,
          M: 'a
{
    #[inline]
    fn clone(&self) -> Self {
        GroupedValues(self.0.clone())
    }

}

impl<'a, K, T, M> Debug for GroupedValues<'a, K, T, M>
    where K: Copy + 'a,
          T: Debug + ?Sized + 'a,
          M: 'a
{
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        fter.debug_list()
            .entry(&self.clone())
            .finish()
    }
}

impl<'a, K, T, M> Iterator for GroupedValues<'a, K, T, M>
    where K: Copy + 'a,
          T: ?Sized + 'a,
          M: 'a
{
    type Item = Group<'a, K, T, M>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(k, pair)| {
            Group {
                inner_iter: pair.1.iter(),
                meta: &pair.0,
                key: *k,
            }
        })
    }
}

pub struct Values<'a, K: 'a, V: 'a>(slice::Iter<'a, (K,V)>);
pub struct ValuesMut<'a, K: 'a, V: 'a>(slice::IterMut<'a, (K,V)>);


//for some reason derive does not work
impl<'a, K: 'a, V: 'a> Clone for Values<'a, K, V> {
    fn clone(&self) -> Self {
        Values(self.0.clone())
    }
}

impl<'a, K: 'a, V: 'a> Iterator for Values<'a, K, V>
    where V: StableDeref
{
    type Item = &'a V::Target;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|&(_, ref v)| {
            &**v
        })
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<'a, K: 'a, V: 'a> ExactSizeIterator for Values<'a, K, V>
    where V: StableDeref
{
    fn len(&self) -> usize {
        self.0.len()
    }
}

impl<'a, K: 'a, V: 'a> Debug for Values<'a, K, V>
    where V: StableDeref, V::Target: Debug
{
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        fter.debug_list().entries(self.clone()).finish()
    }
}


impl<'a, K: 'a, V: 'a> Iterator for ValuesMut<'a, K, V>
    where V: StableDeref + DerefMut //not StableDerefMut!
{
    type Item = &'a mut V::Target;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|&mut (_, ref mut v)| {
            &mut **v
        })
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }

}

impl<'a, K: 'a, V: 'a> ExactSizeIterator for ValuesMut<'a, K, V>
    where V: StableDeref + DerefMut
{
    fn len(&self) -> usize {
        self.0.len()
    }
}

impl<'a, K: 'a, V: 'a> Debug for ValuesMut<'a, K, V>
    where V: StableDeref, V::Target: Debug
{
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        fter.debug_struct("ValuesMut")
            .field("inner_iter", &"..")
            .finish()
    }
}