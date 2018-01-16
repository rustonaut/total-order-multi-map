use std::cell::RefCell;
use std::fmt::{self, Debug};

pub struct DebugIterableOpaque<I> {
    one_use_inner: RefCell<I>
}

impl<I> DebugIterableOpaque<I> {
    pub fn new(one_use_inner: I) -> Self {
        let one_use_inner = RefCell::new(one_use_inner);
        DebugIterableOpaque { one_use_inner }
    }
}
impl<I> Debug for DebugIterableOpaque<I>
    where I: Iterator, I::Item: Debug
{
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        let mut borrow = self.one_use_inner.borrow_mut();
        fter.debug_list().entries(&mut *borrow).finish()
    }
}

#[cfg(test)]
#[macro_export]
macro_rules! assert_ok {
    ($val:expr) => ({
        match $val {
            Ok( res ) => res,
            Err( err ) => panic!( "expected Ok(..) got Err({:?})", err)
        }
    });
    ($val:expr, $ctx:expr) => ({
        match $val {
            Ok( res ) => res,
            Err( err ) => panic!( "expected Ok(..) got Err({:?}) [ctx: {:?}]", err, $ctx)
        }
    });
}