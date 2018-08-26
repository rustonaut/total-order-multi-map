use std::hash::Hash;
use std::cmp::Eq;
use std::fmt::{self, Debug};
use std::collections::hash_map;
use std::ops::DerefMut;
use std::mem;

use stable_deref_trait::StableDeref;
use vec_drain_where::*;

use utils::DebugIterableOpaque;

use super::{TotalOrderMultiMap, EntryValuesMut};

impl<K, V> TotalOrderMultiMap<K, V>
    where K: Hash + Eq + Copy,
          V: StableDeref + DerefMut
{
    /// return a entry for a given key
    pub fn entry(&mut self, key: K) -> Entry<K, V> {
        let vec_data_ref = &mut self.vec_data;
        let map_access_entry = self.map_access.entry(key);
        Entry { vec_data_ref, map_access_entry }
    }
}

pub struct Entry<'a, K, V>
    where K: 'a,
          V: StableDeref + DerefMut + 'a,
{
    vec_data_ref: &'a mut Vec<(K, V)>,
    map_access_entry: hash_map::Entry<'a, K, Vec<*mut V::Target>>
}

impl<'a, K, V> Debug for Entry<'a, K, V>
    where K: Hash + Eq + Copy + Debug + 'a,
          V: StableDeref + DerefMut + 'a,
          V::Target: Debug
{
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        use self::hash_map::Entry::*;
        let dio: Box<Debug> = match self.map_access_entry {
            Occupied(ref o) => {
                Box::new(DebugIterableOpaque::new(o.get().iter().map(|&ptr| unsafe { &*ptr })))
            },
            //SAFE because of TotalOrderMultiMap safty constraints the pointer is always valid
            Vacant(..) => Box::new("[]")
        } ;
        fter.debug_struct("Entry")
            .field("key", &self.key())
            .field("values", &*dio)
            .finish()
    }
}



impl<'a, K, V> Entry<'a, K, V>
    where K: Hash + Eq + Copy + 'a,
          V: StableDeref + DerefMut + 'a
{
    /// access the key used to construct this entry
    pub fn key(&self) -> K {
        *self.map_access_entry.key()
    }

    /// return how many values are associated with this entries key
    pub fn value_count(&self) -> usize {
        use self::hash_map::Entry::*;
        match self.map_access_entry {
            Occupied(ref o) => o.get().len(),
            Vacant(..) => 0
        }
    }

    /// Add a value to the values associated with the given keys.
    pub fn add(self, val: V) -> EntryValuesMut<'a, V::Target> {
        use self::hash_map::Entry::*;
        let mut val = val;

        let Entry { vec_data_ref, map_access_entry } = self;
        let ptr: *mut V::Target = val.deref_mut();
        let key = *map_access_entry.key();

        vec_data_ref.push((key, val));

        let vals = match map_access_entry {
            Occupied(mut oe) => {
                let mut mut_vec = oe.into_mut();
                mut_vec.push(ptr);
                mut_vec
            },
            Vacant(ve) => {
                ve.insert(vec![ptr])

            }
        };

        // Can't use the entries return value as it's &mut Vec<ptr> with last == ptr.
        EntryValuesMut { inner_iter: vals.iter_mut() }
    }

    /// Sets a value for a given key, removing all previous associated values.
    ///
    /// Returns the values previous associated with the key.
    pub fn set(self, val: V) -> Vec<V> {
        use self::hash_map::Entry::*;
        let mut val = val;

        let Entry { vec_data_ref, map_access_entry } = self;
        let ptr: *mut V::Target = val.deref_mut();
        let key = *map_access_entry.key();

        // we can't replace as we need to keep the insertion order
        vec_data_ref.push((key, val));

        let mut nr_of_old_vals =
            match map_access_entry {
                Occupied(mut oe) => {
                    mem::replace(oe.get_mut(), vec![ptr]).len()
                },
                Vacant(ve) => {
                    ve.insert(vec![ptr]);
                    0
                }
            };

        vec_data_ref.e_drain_where(move |&mut (ref k, _)| {
            if nr_of_old_vals > 0 && k == &key {
                nr_of_old_vals -= 1;
                true
            } else {
                false
            }
        }).map(|(_k,v)| v).collect()
    }
}



#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn entry_set_with_prev_vals() {
        let mut map = TotalOrderMultiMap::new();
        map.add("k1", "v1".to_owned());
        map.add("k2", "v2".to_owned());
        map.add("k1", "v3".to_owned());
        map.add("k3", "v4".to_owned());

        let res = map.entry("k1").set("xx".to_owned());
        assert_eq!(vec!["v1".to_owned(), "v3".to_owned()], res);

        assert_eq!(
            vec![ ("k2", "v2"), ("k3", "v4"), ("k1" , "xx") ],
            map.iter().collect::<Vec<_>>()
        );
    }

    #[test]
    fn entry_set_with_no_prev_vals() {
        let mut map = TotalOrderMultiMap::new();
        map.entry("k1").set("xx".to_owned());
        assert_eq!(
            vec![ ("k1" , "xx") ],
            map.iter().collect::<Vec<_>>()
        );
    }

    #[test]
    fn base() {
        let mut map = TotalOrderMultiMap::new();
        map.add("k1", "v1".to_owned());
        map.add("k2", "b".to_owned());
        map.add("k1", "v2".to_owned());


        {
            let entry = map.entry("k1");
            assert_eq!("k1", entry.key());
            assert_eq!(2, entry.value_count());
            entry.add("vX".to_owned());
        }

        assert_eq!(
            ["v1", "b", "v2", "vX"],
            map.values().collect::<Vec<_>>().as_slice()
        );

        assert_eq!(
            ["v1", "v2", "vX"],
            map.get("k1").unwrap().collect::<Vec<_>>().as_slice()
        );

        {
            let entry = map.entry("k99");
            assert_eq!("k99", entry.key());
            assert_eq!(0, entry.value_count());
        }

        {
            let entry = map.entry("k88");
            assert_eq!("k88", entry.key());
            assert_eq!(0, entry.value_count());
            entry.add("end.".to_owned());
        }

        assert_eq!(
            [("k1", "v1"), ("k2", "b"), ("k1", "v2"), ("k1", "vX"), ("k88", "end.")],
            map.iter().collect::<Vec<_>>().as_slice()
        );


    }
}


