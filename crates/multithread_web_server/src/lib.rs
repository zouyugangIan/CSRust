use std::{thread, vec};

pub struct ThreadPool {
    workers: Vec<Worker>,
}

impl ThreadPool {
    ///Create a new ThreadPool
    ///
    /// The size is the number of the threads in the pool.
    ///
    /// #Panics
    ///
    /// The 'new' function will panic if the size is zero.
    pub fn new(size: usize) -> Self {
        assert!(size > 0);

        let mut workers = Vec::with_capacity(size);

        for id in 0..size {
            workers.push(Worker::new(id));
        }

        ThreadPool { workers }
    }

    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
    }
}

// pub fn build(size: usize) -> Result<ThreadPool, PoolCreationError> {
//     if size == 0 {
//         return Err(PoolCreationError::ZeroSize);
//     }
//     Ok(ThreadPool::new(size))
// }

struct Worker {
    id: usize,
    thread: thread::JoinHandle<()>,
}

impl Worker {
    fn new(id: usize) -> Self {
        let thread = thread::spawn(|| {});

        Worker { id, thread }
    }
}
