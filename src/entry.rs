use std::hash::Hash;
use std::cmp::Eq;
use std::fmt::{self, Debug};
use std::collections::hash_map;

use stable_deref_trait::StableDeref;

use utils::DebugIterableOpaque;

use super::TotalOrderMultiMap;

impl<K, V> TotalOrderMultiMap<K, V>
    where K: Hash + Eq + Copy,
          V: StableDeref
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
          V: StableDeref + 'a,
{
    vec_data_ref: &'a mut Vec<(K, V)>,
    map_access_entry: hash_map::Entry<'a, K, Vec<*const V::Target>>
}

impl<'a, K, V> Debug for Entry<'a, K, V>
    where K: Hash + Eq + Copy + Debug + 'a,
          V: StableDeref + 'a,
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
          V: StableDeref + 'a
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

    /// insert a value associating it with the key
    pub fn insert(self, val: V) -> &'a V::Target {
        use self::hash_map::Entry::*;

        let Entry { vec_data_ref, map_access_entry } = self;
        let ptr: *const V::Target = &*val;
        let key = *map_access_entry.key();

        vec_data_ref.push((key, val));

        match map_access_entry {
            Occupied(mut oe) => {
                oe.get_mut().push(ptr);
            },
            Vacant(ve) => {
                ve.insert(vec![ptr]);

            }
        }

        //can't use the entries return value as it's &mut Vec<ptr> with last == ptr
        unsafe { &*ptr }
    }


}



#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn entry() {
        let mut map = TotalOrderMultiMap::new();
        map.insert("k1", "v1");
        map.insert("k2", "b");
        map.insert("k1", "v2");


        {
            let entry = map.entry("k1");
            assert_eq!("k1", entry.key());
            assert_eq!(2, entry.value_count());
            entry.insert("vX");
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
            entry.insert("end.");
        }

        assert_eq!(
            [("k1", "v1"), ("k2", "b"), ("k1", "v2"), ("k1", "vX"), ("k88", "end.")],
            map.iter().collect::<Vec<_>>().as_slice()
        );


    }
}


