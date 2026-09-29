use std::{
    option,
    sync::{Arc, Mutex, mpsc},
    thread,
};

pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: Option<mpsc::Sender<Job>>,
}

type Job = Box<dyn FnOnce() + Send + 'static>;

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

        let (sender, receiver) = mpsc::channel();
        let receiver = Arc::new(Mutex::new(receiver));
        let mut workers = Vec::with_capacity(size);

        for id in 0..size {
            workers.push(Worker::new(id, Arc::clone(&receiver)));
        }

        ThreadPool {
            workers,
            sender: Some(sender),
        }
    }

    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);

        self.sender.as_ref().unwrap().send(job).unwrap();
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        drop(self.sender.take());

        for worker in &mut self.workers.drain(..) {
            println!("Shutting down worker {}", worker.id);

            // worker.thread.join().unwrap();
            if let Err(e) = worker.thread.join() {
                eprintln!("Worker thread join failed:{:?}", e)
            }
        }
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
    fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Job>>>) -> Self {
        let thread = thread::spawn(move || {
            loop {
                let message = receiver.lock().unwrap().recv();

                match message {
                    Ok(job) => {
                        println!("worker {} got a job; executing.", id);
                        job();
                    }
                    Err(_) => {
                        println!("Worker {} got no job; exiting.", id);
                        break;
                    }
                }
            }
        });
        Worker { id, thread }
    }
}
