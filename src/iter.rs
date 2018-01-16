use std::hash::Hash;
use std::cmp::Eq;
use std::slice;
use std::fmt::{ self, Debug };
use std::ops::DerefMut;
use std::iter::ExactSizeIterator;

use stable_deref_trait::StableDeref;

use super::{ Meta, TotalOrderMultiMap };

impl<K, V, M> TotalOrderMultiMap<K, V, M>
    where K: Hash + Eq + Copy,
          V: StableDeref,
          M: Meta
{
    pub fn iter(&self) -> Iter<K, V> {
        Iter {
            vec_iter: self.vec_data.iter()
        }
    }
}

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
          V: StableDeref,
          V::Target: Debug
{
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        fter.debug_list().entries(self.clone()).finish()
    }
}

impl<'a, K, V> Iterator for Iter<'a, K,V>
    where K: Hash + Eq + Copy,
          V: StableDeref
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
          V: StableDeref
{
    fn len(&self) -> usize {
        self.vec_iter.len()
    }
}


impl<K, V, M> TotalOrderMultiMap<K, V, M>
    where K: Hash + Eq + Copy,
          V: StableDeref + DerefMut,
          M: Meta
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

impl<'a, K, V> Iterator for IterMut<'a, K,V>
    where K: Hash + Eq + Copy,
          V: StableDeref + DerefMut
{
    type Item = (K, &'a mut V::Target);

    fn next(&mut self) -> Option<Self::Item> {
        self.vec_iter.next()
            .map( |&mut (ref key, ref mut value)| {
                (*key, &mut **value)
            } )
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
          V: StableDeref+ DerefMut,
          V::Target: Debug
{
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        fter.debug_struct("IterMut")
            .field("inner_iter", &"..")
            .finish()
    }
}