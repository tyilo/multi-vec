pub(crate) trait NonEmptyArray<const N_MINUS_ONE: usize> {
    type Item;
    const N: usize;
}

pub(crate) trait NonEmptyArrayExt<const N_MINUS_ONE: usize>:
    NonEmptyArray<N_MINUS_ONE>
{
    fn arr_split_first(&self) -> (&Self::Item, &[Self::Item; N_MINUS_ONE]);
    fn arr_split_first_mut(&mut self) -> (&mut Self::Item, &mut [Self::Item; N_MINUS_ONE]);
}

impl<const N_MINUS_ONE: usize, const N: usize, T> NonEmptyArrayExt<N_MINUS_ONE> for [T; N]
where
    [T; N]: NonEmptyArray<N_MINUS_ONE, Item = T>,
{
    fn arr_split_first(&self) -> (&Self::Item, &[Self::Item; N_MINUS_ONE]) {
        let (first, rest) = self.as_slice().split_first().unwrap();
        let rest = unsafe { &*rest.as_ptr().cast::<[T; N_MINUS_ONE]>() };
        (first, rest)
    }

    fn arr_split_first_mut(&mut self) -> (&mut Self::Item, &mut [Self::Item; N_MINUS_ONE]) {
        let (first, rest) = self.as_mut_slice().split_first_mut().unwrap();
        let rest = unsafe { &mut *rest.as_mut_ptr().cast::<[T; N_MINUS_ONE]>() };
        (first, rest)
    }
}

macro_rules! impl_non_empty_array {
    ($n:expr) => {
        impl<T> NonEmptyArray<{ $n - 1 }> for [T; $n] {
            type Item = T;
            const N: usize = $n;
        }
    };
}

impl_non_empty_array!(1);
impl_non_empty_array!(2);
impl_non_empty_array!(3);
impl_non_empty_array!(4);
impl_non_empty_array!(5);
impl_non_empty_array!(6);
impl_non_empty_array!(7);
impl_non_empty_array!(8);
impl_non_empty_array!(9);
impl_non_empty_array!(10);
impl_non_empty_array!(11);
impl_non_empty_array!(12);
impl_non_empty_array!(13);
impl_non_empty_array!(14);
impl_non_empty_array!(15);
impl_non_empty_array!(16);

#[cfg(test)]
mod test {
    use super::NonEmptyArrayExt;

    #[test]
    fn test_arr_split_first_1() {
        let arr = [123];
        let (first, remaining) = arr.arr_split_first();
        assert_eq!(*first, 123);
        assert_eq!(*remaining, []);
    }

    #[test]
    fn test_arr_split_first() {
        let arr = [1, 2, 3, 4];
        let (first, remaining) = arr.arr_split_first();
        assert_eq!(*first, 1);
        assert_eq!(*remaining, [2, 3, 4]);
    }

    #[test]
    fn test_arr_split_first_mut_1() {
        let mut arr = [123];
        let (first, remaining) = arr.arr_split_first_mut();
        assert_eq!(*first, 123);
        assert_eq!(*remaining, []);

        *first = 456;
        assert_eq!(arr, [456]);
    }

    #[test]
    fn test_arr_split_first_mut() {
        let mut arr = [1, 2, 3, 4];
        let (first, remaining) = arr.arr_split_first_mut();
        assert_eq!(*first, 1);
        assert_eq!(*remaining, [2, 3, 4]);

        *first = 123;
        remaining[1] = 456;
        assert_eq!(arr, [123, 2, 456, 4]);
    }
}
