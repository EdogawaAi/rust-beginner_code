use crossbeam_channel;
use std::{thread, time};

fn parallel_map<T, U, F>(mut input_vec: Vec<T>, num_threads: usize, f: F) -> Vec<U>
where
    F: FnOnce(T) -> U + Send + Copy + 'static,
    T: Send + 'static,
    U: Send + 'static + Default,
{
    let mut output_vec: Vec<U> = Vec::with_capacity(input_vec.len());
    // TODO: implement parallel map!
    let (input_sender, input_receiver) = crossbeam_channel::unbounded();
    let (output_sender, output_receiver) = crossbeam_channel::unbounded();

    let mut threads = Vec::new();
    for _ in 0..num_threads {
        let input_receiver = input_receiver.clone();
        let output_sender = output_sender.clone();
        threads.push(thread::spawn(move || {
            while let Ok ((index, item)) = input_receiver.recv(){
                let result = f(item);
                output_sender.send((index, result)).unwrap();
            }
        }));
    }

    for (index, item) in input_vec.into_iter().enumerate() {
        input_sender.send((index, item)).unwrap();
    }

    drop(input_sender);

    for _ in 0..output_vec.len() {
        let (index, result) = output_receiver.recv().unwrap();
        output_vec[index] = result;
    }

    for thread in threads {
        thread.join().expect("Panic occurred in thread");
    }

    output_vec
}

fn main() {
    let v = vec![6, 7, 8, 9, 10, 1, 2, 3, 4, 5, 12, 18, 11, 5, 20];
    let squares = parallel_map(v, 10, |num| {
        println!("{} squared is {}", num, num * num);
        thread::sleep(time::Duration::from_millis(500));
        num * num
    });
    println!("squares: {:?}", squares);
}
