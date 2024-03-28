mod array;
use std::{
    fmt::Debug,
    ops::{Deref, DerefMut, Index, IndexMut},
    sync::Mutex,
};

use itertools::Itertools;

use crate::array::NonEmptyArrayExt;

trait Placeholder {}
impl<T> Placeholder for T {}

struct Refs(Mutex<Vec<*mut dyn Placeholder>>);
unsafe impl Send for Refs {}
unsafe impl Sync for Refs {}

impl Refs {
    fn new() -> Self {
        Self(Default::default())
    }

    fn add(&self, ptr: *mut dyn Placeholder) {
        self.0.lock().unwrap().push(ptr);
    }
}

impl Drop for Refs {
    fn drop(&mut self) {
        for ptr in self.0.lock().unwrap().iter() {
            _ = unsafe { Box::from_raw(*ptr) };
        }
    }
}

struct DebugSlice<'a, T> {
    slice: &'a [T],
    offsets: &'a [usize],
}

impl<T: Debug> Debug for DebugSlice<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Some((offset, offsets)) = self.offsets.split_first() else {
            return f.debug_list().entries(self.slice).finish();
        };
        let mut list = f.debug_list();

        let mut i = 0;
        while i * offset < self.slice.len() {
            let slice = &self.slice[i * offset..(i + 1) * offset];
            if slice.is_empty() {
                break;
            }
            list.entry(&DebugSlice { slice, offsets });
            i += 1;
        }

        list.finish()
    }
}

#[derive(Clone, Copy)]
pub struct MultiVecRef<const N: usize, T> {
    slice: *mut [T],
    offsets: *const [usize; N],
    refs: *const Refs,
}

impl<const N: usize, T> MultiVecRef<N, T> {
    fn slice(&self) -> &[T] {
        unsafe { &*self.slice }
    }

    fn slice_mut(&mut self) -> &mut [T] {
        unsafe { &mut *self.slice }
    }

    fn offsets(&self) -> &[usize; N] {
        unsafe { &*self.offsets }
    }

    pub fn size(&self) -> usize {
        self.slice().len() / self.offsets().first().cloned().unwrap_or(1)
    }
}

impl<const N: usize, T> Debug for MultiVecRef<N, T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple(&format!("MultiVecRef<{N}>"))
            .field(&DebugSlice {
                slice: self.slice(),
                offsets: self.offsets(),
            })
            .finish()
    }
}

pub struct MultiVec<const N: usize, T> {
    inner: Vec<T>,
    offsets: [usize; N],
    refs: Refs,
}

impl<const N: usize, T: Clone> Clone for MultiVec<N, T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            offsets: self.offsets,
            refs: Refs::new(),
        }
    }
}

impl<const N: usize, T: PartialEq> PartialEq for MultiVec<N, T> {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner && self.offsets == other.offsets
    }
}

impl<const N: usize, T: Eq> Eq for MultiVec<N, T> {}

impl<const N: usize, T> Debug for MultiVec<N, T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple(&format!("MultiVec<{N}>"))
            .field(&DebugSlice {
                slice: &self.inner,
                offsets: &self.offsets,
            })
            .finish()
    }
}

impl<const N: usize, T> MultiVec<N, T> {
    pub fn default(outer_size: usize, sizes: [usize; N]) -> Self
    where
        T: Default + Clone,
    {
        Self::from_fn(outer_size, sizes, |_, _| Default::default())
    }

    pub fn from_fn(
        outer_size: usize,
        mut sizes: [usize; N],
        f: impl Fn(usize, [usize; N]) -> T,
    ) -> Self {
        let mut prod = 1;
        let offsets = {
            sizes.reverse();
            let mut offsets = sizes.map(|n| {
                prod *= n;
                prod
            });
            offsets.reverse();
            sizes.reverse();
            offsets
        };

        let inner = [outer_size]
            .into_iter()
            .chain(sizes)
            .map(|n| 0..n)
            .multi_cartesian_product()
            .map(|mut indices| {
                let first = indices.remove(0);
                f(first, indices.try_into().unwrap())
            })
            .collect();

        Self {
            offsets,
            inner,
            refs: Refs::new(),
        }
    }
}

impl<const N: usize, T> Deref for MultiVec<N, T> {
    type Target = MultiVecRef<N, T>;

    fn deref(&self) -> &Self::Target {
        let ptr = Box::into_raw(Box::new(MultiVecRef {
            slice: self.inner.as_slice() as *const _ as *mut _,
            offsets: &self.offsets,
            refs: &self.refs,
        }));
        self.refs.add(ptr as *mut dyn Placeholder);
        unsafe { &*ptr }
    }
}

impl<const N: usize, T> DerefMut for MultiVec<N, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        let ptr = Box::into_raw(Box::new(MultiVecRef {
            slice: self.inner.as_mut_slice() as *mut _,
            offsets: &self.offsets,
            refs: &self.refs,
        }));
        self.refs.add(ptr as *mut dyn Placeholder);
        unsafe { &mut *ptr }
    }
}

impl<T> Index<usize> for MultiVecRef<0, T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        &self.slice()[index]
    }
}

impl<T> IndexMut<usize> for MultiVecRef<0, T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.slice_mut()[index]
    }
}

macro_rules! impl_index {
    ($n:expr) => {
        impl<T: Debug> Index<usize> for MultiVecRef<$n, T> {
            type Output = MultiVecRef<{ $n - 1 }, T>;

            fn index(&self, index: usize) -> &Self::Output {
                let (offset, offsets) = self.offsets().arr_split_first();
                let ptr = Box::into_raw(Box::new(MultiVecRef {
                    slice: &self.slice()[index * offset..(index + 1) * offset] as *const _
                        as *mut _,
                    offsets,
                    refs: self.refs,
                }));
                let refs = unsafe { &*self.refs };
                refs.add(ptr as *mut dyn Placeholder);
                unsafe { &*ptr }
            }
        }

        impl<T: Debug> IndexMut<usize> for MultiVecRef<$n, T> {
            fn index_mut(&mut self, index: usize) -> &mut Self::Output {
                let MultiVecRef {
                    slice,
                    offsets,
                    refs,
                } = self;
                let slice = unsafe { &mut **slice };
                let offsets = unsafe { &**offsets };
                let (offset, offsets) = offsets.arr_split_first();
                let offset = *offset;
                let ptr = Box::into_raw(Box::new(MultiVecRef {
                    slice: &mut slice[index * offset..(index + 1) * offset] as *mut _,
                    offsets,
                    refs: *refs,
                }));
                let refs = unsafe { &*self.refs };
                refs.add(ptr as *mut dyn Placeholder);
                unsafe { &mut *ptr }
            }
        }
    };
}

impl_index!(1);
impl_index!(2);
impl_index!(3);
impl_index!(4);
impl_index!(5);
impl_index!(6);
impl_index!(7);
impl_index!(8);
impl_index!(9);

#[cfg(test)]
mod test {
    use proptest::prelude::*;

    use crate::MultiVec;

    const _: () = {
        const fn assert_send<T: Send>() {}
        const fn assert_sync<T: Sync>() {}
        assert_send::<MultiVec<0, u8>>();
        assert_sync::<MultiVec<0, u8>>();
    };

    struct NonClone<T>(T);

    #[test]
    fn test_thread_move() {
        let m = MultiVec::<2, _>::from_fn(3, [4, 5], |i, [j, k]| NonClone((i, j, k)));
        std::thread::scope(|scope| {
            scope.spawn(move || {
                drop(m);
            });
        });
    }

    #[test]
    fn test_thread_ref() {
        let m = MultiVec::<2, _>::from_fn(3, [4, 5], |i, [j, k]| NonClone((i, j, k)));
        std::thread::scope(|scope| {
            scope.spawn(|| &m);
            scope.spawn(|| &m);
        });
    }

    proptest! {
        #[test]
        fn test_from_fn_0(n in 0..10usize) {
            let m = MultiVec::<0, _>::from_fn(n, [], |i, _| i);
            for (i, v) in m.inner.into_iter().enumerate() {
                prop_assert_eq!(i, v);
            }
        }

        #[test]
        fn test_from_fn_1(n1 in 0..10usize, n2 in 0..10usize) {
            let m = MultiVec::<1, _>::from_fn(n1, [n2], |i1, [i2]| (i1, i2));
            for i1 in 0..n1 {
                for i2 in 0..n2 {
                    prop_assert_eq!((i1, i2), m[i1][i2]);
                }
            }
        }

        #[cfg_attr(miri, ignore)]
        #[test]
        fn test_from_fn_2(outer_size in 0..10usize, sizes in prop::array::uniform2(0..10usize)) {
            let m1 = MultiVec::<2, _>::from_fn(outer_size, sizes, |i1, [i2, i3]| (i1, i2, i3));
            for i1 in 0..outer_size {
                for i2 in 0..sizes[0] {
                    for i3 in 0..sizes[1] {
                        prop_assert_eq!((i1, i2, i3), m1[i1][i2][i3]);
                    }
                }
            }

            let mut m2 = MultiVec::<2, _>::default(outer_size, sizes);
            for i1 in 0..outer_size {
                for i2 in 0..sizes[0] {
                    for i3 in 0..sizes[1] {
                        m2[i1][i2][i3] = (i1, i2, i3);
                    }
                }
            }

            prop_assert_eq!(m1, m2);
        }
    }
}
