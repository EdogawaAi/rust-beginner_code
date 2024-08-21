/* The following exercises were borrowed from Will Crichton's CS 242 Rust lab. */

use std::collections::HashSet;
use std::ops::Index;

fn main() {
    println!("Hi! Try running \"cargo test\" to run tests.");
}

fn add_n(v: Vec<i32>, n: i32) -> Vec<i32> {
    let mut result: Vec<i32> = Vec::new();
    for i in v.iter(){
        result.push(*i + n);
    }
    result
}

fn add_n_inplace(v: &mut Vec<i32>, n: i32) {
    for mut i in v.iter_mut()  {
        *i += n;
    }
}

fn dedup(v: &mut Vec<i32>) {
    // unimplemented!()
    let mut vec = HashSet::new();
    let mut i = 0;
    while i < v.len(){
        if vec.contains(&v[i]){
            v.remove(i);
        }
        else{
            vec.insert(v[i]);
            i += 1;
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_add_n() {
        assert_eq!(add_n(vec![1], 2), vec![3]);
    }

    #[test]
    fn test_add_n_inplace() {
        let mut v = vec![1];
        add_n_inplace(&mut v, 2);
        assert_eq!(v, vec![3]);
    }

    #[test]
    fn test_dedup() {
        let mut v = vec![3, 1, 0, 1, 4, 4];
        dedup(&mut v);
        assert_eq!(v, vec![3, 1, 0, 4]);
    }
}
