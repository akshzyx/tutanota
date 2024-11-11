/// split a given vector of elements into a vector of chunks not exceeding max_size, where the
/// chunks size is calculated by summing up the elements sizes as given by the sizer function.
///
/// the number of chunks is not guaranteed to be optimal.
pub fn reduce_to_chunks<T>(mut seq: Vec<T>, max_size: usize, sizer: impl Fn(&T) -> usize) -> Result<Vec<Vec<T>>, ()> {
    let mut output: Vec<Vec<T>> = Vec::new();
    loop {
        let mut current_chunk_size = 0_usize;
        if seq.is_empty() {
            break;
        }
        let mut split_count: usize = 0_usize;

        'chunker: for element in seq.iter() {
            let size = sizer(element);
            if size > max_size {
                return Err(());
            }
            if current_chunk_size.saturating_add(size) > max_size {
                // chunk is full - the next element
                break 'chunker;
            } else {
                current_chunk_size = current_chunk_size.saturating_add(size);
                split_count += 1;
            }
        };
        let len = seq.len();
        let rest = seq.split_off(split_count);
        output.push(seq);
        seq = rest;
        assert_eq!(seq.len() + split_count, len);
    }

    Ok(output)
}


#[cfg(test)]
mod tests {
    use crate::reduce_to_chunks::reduce_to_chunks;

    #[test]
    fn reduce_to_chunks_simple() {
        assert_eq!(vec![
            vec![1, 2, 3],
            vec![4],
            vec![5],
            vec![6]
        ],
                   reduce_to_chunks::<usize>(vec![1, 2, 3, 4, 5, 6], 6, |item| { *item }).unwrap()
        );
    }

    #[test]
    fn reduce_to_chunks_no_split() {
        assert_eq!(vec![
            vec![1, 2, 3, 4, 5, 6],
        ],
                   reduce_to_chunks::<usize>(vec![1, 2, 3, 4, 5, 6], 21, |item| { *item }).unwrap()
        );
    }

    fn reduce_to_chunks_empty() {
        assert_eq!(
            Vec::<Vec<usize>>::new(),
            reduce_to_chunks::<usize>(vec![], 0, |item| { *item }).unwrap()
        );
    }

    fn split_too_big() {
        assert_eq!(
            Err(()),
            reduce_to_chunks::<usize>(vec![1, 10, 11], 2, |item| { *item })
        );
    }
}