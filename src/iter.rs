//TODO merge file with map_iter
use std::hash::Hash;
use std::cmp::Eq;
use std::slice;
use std::fmt::{ self, Debug };
use std::ops::DerefMut;
use std::iter::ExactSizeIterator;

use stable_deref_trait::StableDeref;

use super::TotalOrderMultiMap;

impl<K, V> TotalOrderMultiMap<K, V>
    where K: Hash + Eq + Copy,
          V: StableDeref + DerefMut,
{
    /// Returns a iterator over the key value pairs in insertion order.
    ///
    /// As this is a multi map a key can appear more than one time.
    pub fn iter(&self) -> Iter<K, V> {
        Iter {
            vec_iter: self.vec_data.iter()
        }
    }

}

/// Iterator over TotalOrderMultimap in insertion order.
pub struct Iter<'a, K: 'a, V: 'a> {
    vec_iter: slice::Iter<'a, (K, V)>
}

impl<'a, K, V> Clone for Iter<'a, K, V> {
    fn clone(&self) -> Self {
        Iter { vec_iter: self.vec_iter.clone() }
    }
}

impl<'a, K, V> Debug for Iter<'a, K, V>
    where K: Hash + Eq + Copy + Debug,
          V: StableDeref + DerefMut,
          V::Target: Debug
{
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        fter.debug_list().entries(self.clone()).finish()
    }
}

impl<'a, K, V> Iterator for Iter<'a, K,V>
    where K: Hash + Eq + Copy,
          V: StableDeref + DerefMut
{
    type Item = (K, &'a V::Target);

    fn next(&mut self) -> Option<Self::Item> {
        self.vec_iter.next()
            .map( |&(ref key, ref value)| {
                (*key, &**value)
            } )
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.vec_iter.size_hint()
    }
}

impl<'a, K, V> ExactSizeIterator for Iter<'a, K, V>
    where K: Hash + Eq + Copy,
          V: StableDeref + DerefMut
{
    fn len(&self) -> usize {
        self.vec_iter.len()
    }
}


impl<K, V> TotalOrderMultiMap<K, V>
    where K: Hash + Eq + Copy,
          V: StableDeref + DerefMut
{
    pub fn iter_mut(&mut self) -> IterMut<K, V> {
        IterMut {
            vec_iter: self.vec_data.iter_mut()
        }
    }
}

pub struct IterMut<'a, K: 'a, V: 'a> {
    vec_iter: slice::IterMut<'a, (K, V)>
}

impl<'a, K: 'a, V: 'a> From<IterMut<'a, K, V>> for Iter<'a, K, V> {
    fn from(valsmut: IterMut<'a, K, V>) -> Self {
        let IterMut { vec_iter } = valsmut;
        let as_slice = vec_iter.into_slice();
        let vec_iter = as_slice.iter();
        Iter { vec_iter }
    }
}

impl<'a, K, V> Iterator for IterMut<'a, K, V>
    where K: Hash + Eq + Copy,
          V: StableDeref + DerefMut
{
    type Item = (K, &'a mut V::Target);

    fn next(&mut self) -> Option<Self::Item> {
        self.vec_iter.next()
            .map(|&mut (ref key, ref mut value)| {
                (*key, &mut **value)
            })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.vec_iter.size_hint()
    }
}

impl<'a, K, V> ExactSizeIterator for IterMut<'a, K, V>
    where K: Hash + Eq + Copy,
          V: StableDeref + DerefMut
{
    fn len(&self) -> usize {
        self.vec_iter.len()
    }
}

impl<'a, K, V> Debug for IterMut<'a, K, V>
    where K: Hash + Eq + Copy + Debug,
          V: StableDeref + DerefMut+ DerefMut,
          V::Target: Debug
{
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        fter.write_str("IterMut { .. }")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_iter_mut_to_non_mut() {
        let mut map = TotalOrderMultiMap::new();
        map.add("k1", "v1".to_owned());
        let iter: Iter<_,_> = map.iter_mut().into();
        assert_eq!(
            vec![("k1", "v1")],
            iter.collect::<Vec<_>>()
        )
    }

}
