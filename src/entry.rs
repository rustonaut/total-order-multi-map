use std::hash::Hash;
use std::cmp::Eq;
use std::fmt::{self, Debug};
use std::collections::hash_map;

use stable_deref_trait::StableDeref;

use utils::DebugIterableOpaque;

use super::{ TotalOrderMultiMap, Meta };

//TODO add meta
impl<K, V, M> TotalOrderMultiMap<K, V, M>
    where K: Hash + Eq + Copy,
          V: StableDeref,
          M: Meta
{
    pub fn entry(&mut self, key: K) -> Entry<K, V, M> {
        let vec_data_ref = &mut self.vec_data;
        let map_access_entry = self.map_access.entry(key);
        Entry { vec_data_ref, map_access_entry }
    }
}

pub struct Entry<'a, K, V, M>
    where K: 'a,
          V: StableDeref + 'a,
          M: 'a
{
    vec_data_ref: &'a mut Vec<(K, V)>,
    map_access_entry: hash_map::Entry<'a, K, (M, Vec<*const V::Target>)>
}

impl<'a, K, V, M> Debug for Entry<'a, K, V, M>
    where K: Hash + Eq + Copy + Debug + 'a,
          V: StableDeref + 'a,
          V::Target: Debug,
          M: Meta
{
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        use self::hash_map::Entry::*;
        let dio: Box<Debug> = match self.map_access_entry {
            Occupied(ref o) => {
                Box::new(DebugIterableOpaque::new(o.get().1.iter().map(|&ptr| unsafe { &*ptr })))
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



impl<'a, K, V, M> Entry<'a, K, V, M>
    where K: Hash + Eq + Copy + 'a,
          V: StableDeref + 'a,
          M: Meta
{
    pub fn key(&self) -> K {
        *self.map_access_entry.key()
    }

    pub fn meta(&self) -> Option<&M> {
        use self::hash_map::Entry::*;
        match self.map_access_entry {
            Occupied(ref o) => Some(&o.get().0),
            Vacant(..) => None
        }
    }

    pub fn meta_mut(&mut self) -> Option<&mut M> {
        use self::hash_map::Entry::*;
        match self.map_access_entry {
            Occupied(ref mut o) => Some(&mut o.get_mut().0),
            Vacant(..) => None
        }
    }

    pub fn value_count(&self) -> usize {
        use self::hash_map::Entry::*;
        match self.map_access_entry {
            Occupied(ref o) => o.get().1.len(),
            Vacant(..) => 0
        }
    }

    pub fn insert(self, val: V, meta: M) -> Result<(), M::MergeError>{
        let Entry { vec_data_ref, map_access_entry } = self;
        // 1. see if we can update and if so update the meta
        // 2. update vec_data
        // 3. only then insert ptr
        use self::hash_map::Entry::*;
        let ptr: *const V::Target = &*val;
        let key = *map_access_entry.key();
        match map_access_entry {
            Occupied(mut oe) => {
                let pair = oe.get_mut();
                pair.0.check_update(&meta)?;
                pair.0.update(meta);
                vec_data_ref.push((key, val));
                pair.1.push(ptr);
                Ok(())
            },
            Vacant(ve) => {
                vec_data_ref.push((key, val));
                ve.insert((meta, vec![ptr]));
                Ok(())
            }
        }
    }


}



#[cfg(test)]
mod test {
    use super::*;
    use super::super::NoMeta;

    #[test]
    fn entry() {
        let mut map = TotalOrderMultiMap::new();
        assert_ok!(map.insert("k1", "v1", NoMeta));
        assert_ok!(map.insert("k2", "b", NoMeta));
        assert_ok!(map.insert("k1", "v2", NoMeta));


        {
            let entry = map.entry("k1");
            assert_eq!("k1", entry.key());
            assert_eq!(2, entry.value_count());
            assert_ok!(entry.insert("vX", NoMeta));
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
            assert_ok!(entry.insert("end.", NoMeta));
        }

        assert_eq!(
            [("k1", "v1"), ("k2", "b"), ("k1", "v2"), ("k1", "vX"), ("k88", "end.")],
            map.iter().collect::<Vec<_>>().as_slice()
        );


    }
}


