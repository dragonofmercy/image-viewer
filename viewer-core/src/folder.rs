/// Return a copy of `v` without the element at `index`; an out-of-range index returns an unchanged copy.
pub fn remove_at_index<T: Clone>(v: &[T], index: isize) -> Vec<T> {
    let mut out = v.to_vec();
    if let Ok(i) = usize::try_from(index)
        && i < out.len()
    {
        out.remove(i);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_middle_first_and_last() {
        let v = ["a", "b", "c"];
        assert_eq!(remove_at_index(&v, 1), ["a", "c"]);
        assert_eq!(remove_at_index(&v, 0), ["b", "c"]);
        assert_eq!(remove_at_index(&v, 2), ["a", "b"]);
    }

    #[test]
    fn out_of_range_is_unchanged() {
        let v = ["a", "b", "c"];
        assert_eq!(remove_at_index(&v, -1), v);
        assert_eq!(remove_at_index(&v, 3), v);
    }
}
