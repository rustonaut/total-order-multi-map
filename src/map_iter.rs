use std::collections::hash_map;
use std::slice;
use std::hash::Hash;
use std::cmp::Eq;
use std::ops::DerefMut;
use std::fmt::{self, Debug};
use std::iter::{Iterator, ExactSizeIterator};

use stable_deref_trait::StableDeref;


use super::TotalOrderMultiMap;


impl<K, V> TotalOrderMultiMap<K, V>
    where K: Hash+Eq+Copy,
          V: StableDeref + DerefMut
{
    /// Return an iterator the keys of this multi map.
    ///
    /// each key will only be returned once, there is
    /// no specific order in which they keys are returned
    pub fn keys(&self) -> Keys<K, V::Target> {
        Keys { inner_iter: self.map_access.keys() }
    }

    /// Returns a iterator over all values grouped by key.
    pub fn group_iter(&self) -> GroupedValues<K, V::Target> {
        GroupedValues { inner_iter: self.map_access.iter() }
    }

    /// Returns a iterator over all values grouped by key
    pub fn group_iter_mut(&mut self) -> GroupedValuesMut<K, V::Target> {
        GroupedValuesMut { inner_iter: self.map_access.iter_mut() }
    }

    /// Returns a iterator over the inner-values in this multi map.
    ///
    /// Inner-Values are returned in the order they where inserted into
    /// the map. Note that
    pub fn values(&self) -> Values<K, V> {
        Values { inner_iter: self.vec_data.iter() }
    }


    /// Returns a iterator over the values in this multi map.
    pub fn values_mut(&mut self) -> ValuesMut<K, V> {
        ValuesMut { inner_iter: self.vec_data.iter_mut() }
    }
}

#[derive(Clone)]
pub struct Keys<'a, K: 'a, T: ?Sized + 'a> {
    inner_iter: hash_map::Keys<'a, K, Vec<*mut T>>
}

impl<'a, K: Debug+'a, T: 'a> Debug for Keys<'a, K, T> {
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        self.inner_iter.fmt(fter)
    }
}

impl<'a, K: 'a, T: 'a> Iterator for Keys<'a, K, T>
    where K: Copy
{
    type Item = K;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner_iter.next().map(|&k|k)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner_iter.size_hint()
    }
}

impl<'a, K, T> ExactSizeIterator for Keys<'a, K, T>
    where K: Copy + 'a, T: 'a
{
    #[inline]
    fn len(&self) -> usize {
        self.inner_iter.len()
    }
}

//TODO consider merging this with EntryValues
/// A iterator over `&V::Target` values returned by `GroupedValues`.
///
/// It will iterate over all values associated with a specific key
/// in the order they where inserted.
pub struct Group<'a, K, T>
    where K: Copy + 'a,
          T: ?Sized + 'a,
{
    /// Note: we have a & to an *mut so we can only use it as *const/&
    inner_iter: slice::Iter<'a, *mut T>,
    key: K
}

impl<'a, K, T> Clone for Group<'a, K, T>
    where K: Copy + 'a,
          T: ?Sized + 'a,
{
    fn clone(&self) -> Self {
        Group {
            key: self.key.clone(),
            inner_iter: self.inner_iter.clone(),
        }
    }
}

impl<'a, K, T> Group<'a, K, T>
    where K: Copy + 'a,
          T: ?Sized + 'a,
{
    pub fn key(&self) -> K {
        self.key
    }
}

impl<'a, K, T> Iterator for Group<'a, K, T>
    where K: Copy + 'a,
          T: ?Sized + 'a
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

/// A iterator over `&mut V::Target` values returned by `GroupedValues`.
///
/// It will iterate over all values associated with a specific key
/// in the order they where inserted.
pub struct GroupMut<'a, K, T>
    where K: Copy + 'a,
          T: ?Sized + 'a,
{
    /// Note: we have a & to an *mut so we can only use it as *const/&
    inner_iter: slice::IterMut<'a, *mut T>,
    key: K
}

impl<'a, K, T> GroupMut<'a, K, T>
    where K: Copy + 'a,
          T: ?Sized + 'a,
{
    pub fn key(&self) -> K {
        self.key
    }
}

impl<'a, K, T> Iterator for GroupMut<'a, K, T>
    where K: Copy + 'a,
          T: ?Sized + 'a
{
    type Item = &'a mut T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        //SAFE see TotalOrderMultiMap safety guarantees/constraints
        self.inner_iter.next().map(|&mut ptr| unsafe { &mut *ptr } )
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner_iter.size_hint()
    }
}


impl<'a, K, T> ExactSizeIterator for GroupMut<'a, K, T>
    where K: Copy + 'a,
          T: ?Sized + 'a,
{
    #[inline]
    fn len(&self) -> usize {
        self.inner_iter.len()
    }
}

impl<'a, K, T> Debug for Group<'a, K, T>
    where K: Debug + Copy + 'a,
          T: Debug + ?Sized + 'a,
{
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        fter.write_str("Group { .. }")
    }
}


/// An iterator of Groups (no fixed iteration order).
pub struct GroupedValues<
    'a,
    K: Copy + 'a,
    T: ?Sized + 'a,
> {
    /// Note: we have a `&` to an `*mut` so we can only use it as `*const`
    inner_iter: hash_map::Iter<'a, K, Vec<*mut T>>
}

impl<'a, K, T> Clone for GroupedValues<'a, K, T>
    where K: Copy + 'a,
          T: ?Sized + 'a,
{
    #[inline]
    fn clone(&self) -> Self {
        GroupedValues { inner_iter: self.inner_iter.clone() }
    }

}

impl<'a, K, T> Debug for GroupedValues<'a, K, T>
    where K: Copy + 'a,
          T: Debug + ?Sized + 'a,
{
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        fter.debug_list()
            .entry(&self.clone())
            .finish()
    }
}

impl<'a, K, T> Iterator for GroupedValues<'a, K, T>
    where K: Copy + 'a,
          T: ?Sized + 'a,
{
    type Item = Group<'a, K, T>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner_iter.next().map(|(k, values)| {
            Group {
                inner_iter: values.iter(),
                key: *k,
            }
        })
    }
}

/// An iterator of Groups (no fixed iteration order).
pub struct GroupedValuesMut<
    'a,
    K: Copy + 'a,
    T: ?Sized + 'a,
> {
    inner_iter: hash_map::IterMut<'a, K, Vec<*mut T>>
}

impl<'a, K, T> Debug for GroupedValuesMut<'a, K, T>
    where K: Copy + 'a,
          T: Debug + ?Sized + 'a,
{
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        fter.write_str("GroupedValuesMut { .. }")
    }
}

impl<'a, K, T> Iterator for GroupedValuesMut<'a, K, T>
    where K: Copy + 'a,
          T: ?Sized + 'a,
{
    type Item = GroupMut<'a, K, T>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner_iter.next().map(|(k, values)| {
            GroupMut {
                inner_iter: values.iter_mut(),
                key: *k,
            }
        })
    }
}



pub struct Values<'a, K: 'a, V: 'a> {
    inner_iter: slice::Iter<'a, (K, V)>
}

pub struct ValuesMut<'a, K: 'a, V: 'a> {
    inner_iter: slice::IterMut<'a, (K, V)>
}

impl<'a, K: 'a, V: 'a> From<ValuesMut<'a, K, V>> for Values<'a, K, V> {
    fn from(valsmut: ValuesMut<'a, K, V>) -> Self {
        let ValuesMut { inner_iter } = valsmut;
        let as_slice = inner_iter.into_slice();
        let inner_iter = as_slice.iter();
        Values { inner_iter }
    }
}

impl<'a, K: 'a, V: 'a> Clone for Values<'a, K, V> {
    fn clone(&self) -> Self {
        Values { inner_iter: self.inner_iter.clone() }
    }
}

impl<'a, K: 'a, V: 'a> Iterator for Values<'a, K, V>
    where V: StableDeref + DerefMut
{
    type Item = &'a V::Target;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner_iter.next()
            .map(|&(_, ref v)| &**v)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner_iter.size_hint()
    }
}

impl<'a, K: 'a, V: 'a> ExactSizeIterator for Values<'a, K, V>
    where V: StableDeref + DerefMut
{
    fn len(&self) -> usize {
        self.inner_iter.len()
    }
}

impl<'a, K: 'a, V: 'a> Debug for Values<'a, K, V>
    where V: StableDeref + DerefMut, V::Target: Debug
{
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        fter.debug_list().entries(self.clone()).finish()
    }
}


impl<'a, K: 'a, V: 'a> Iterator for ValuesMut<'a, K, V>
    where V: StableDeref + DerefMut
{
    type Item = &'a mut V::Target;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner_iter.next().map(|&mut (_, ref mut v)| {
            &mut **v
        })
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner_iter.size_hint()
    }

}

impl<'a, K: 'a, V: 'a> ExactSizeIterator for ValuesMut<'a, K, V>
    where V: StableDeref + DerefMut
{
    fn len(&self) -> usize {
        self.inner_iter.len()
    }
}

impl<'a, K: 'a, V: 'a> Debug for ValuesMut<'a, K, V>
    where V: StableDeref + DerefMut, V::Target: Debug
{
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        fter.write_str("ValuesMut { .. }")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_mut_values_to_non_mut() {
        let mut map = TotalOrderMultiMap::new();
        map.add("k1", "v1".to_owned());
        map.add("k0", "v2".to_owned());

        let iter: Values<_, _> = map.values_mut().into();
        assert_eq!(
            vec!["v1", "v2"],
            iter.collect::<Vec<_>>()
        )
    }
}