pub fn assert_eq_vec<T: PartialEq>(a: Vec<T>, b: Vec<T>) {
    let matching = a.iter().zip(&b).filter(|&(a, b)| a == b).count();
    assert_eq!(a.len(), matching);
    assert_eq!(a.len(), b.len());
}
