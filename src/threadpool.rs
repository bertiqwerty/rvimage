use std::{
    fmt::Debug,
    sync::mpsc::{self, Receiver, RecvError, SendError, Sender},
    thread,
    time::{self, Instant},
};

use log::{error, info};

type Job<T> = Box<dyn FnOnce() -> T + Send + 'static>;
type RecvResult<T> = Result<T, RecvError>;
type SendResult<T, U> = Result<T, SendError<U>>;
struct ThreadPool<T: Debug + Clone + Send + 'static> {
    txs_to_pool: Vec<Sender<(usize, Job<T>)>>,
    rx_from_pool: Receiver<RecvResult<(usize, T)>>,
    next_thread: usize,
    job_id: usize,
    result_queue: Vec<(usize, T)>,
}
impl<T: Debug + Clone + Send + 'static> ThreadPool<T> {
    pub fn new(n_threads: usize) -> Self {
        let mut txs_to_pool = Vec::with_capacity(n_threads);
        let (tx_from_pool, rx_from_pool) = mpsc::channel();
        for idx_thread in 0..n_threads {
            let (tx_to_pool, rx_to_pool) = mpsc::channel();
            txs_to_pool.push(tx_to_pool);
            let tx = tx_from_pool.clone();
            thread::spawn(move || {
                println!("spawning thread {}", idx_thread);
                loop {
                    let send_result =
                        tx.send(rx_to_pool.recv().map(|(i, f): (usize, Job<T>)| (i, f())));
                    match send_result {
                        Ok(_) => {
                            println!("thread {} send a result.", idx_thread);
                        }
                        Err(_) => {
                            println!("thread {} terminated. receiver gone.", idx_thread);
                            return ();
                        }
                    }
                }
            });
        }
        ThreadPool {
            txs_to_pool,
            rx_from_pool,
            next_thread: 0usize,
            job_id: 0usize,
            result_queue: vec![],
        }
    }

    pub fn result(&mut self, job_id: usize) -> Option<T> {
        self.result_queue
            .extend(self.rx_from_pool.try_iter().flat_map(|received| received));
        println!("looking for {} in {:?}", job_id, self.result_queue);
        let result = self
            .result_queue
            .iter()
            .enumerate()
            .find(|(_, (jid, _))| job_id == *jid);
        println!("found {:?}", result);

        match result {
            None => None,
            Some((vec_idx, _)) => {
                let (_, v) = self.result_queue.remove(vec_idx);
                Some(v)
            }
        }
    }

    pub fn apply(&mut self, f: Job<T>) -> SendResult<usize, (usize, Job<T>)> {
        self.job_id += 1;
        if self.next_thread == self.txs_to_pool.len() {
            self.next_thread = 0;
        }
        println!("sending id {:?}", self.job_id);
        self.txs_to_pool[self.next_thread].send((self.job_id, f))?;
        self.next_thread += 1;

        Ok(self.job_id)
    }
}

#[test]
fn test() -> SendResult<(), (usize, Job<usize>)> {
    let mut tp = ThreadPool::new(4);

    fn apply_job(res: usize, tp: &mut ThreadPool<usize>) -> SendResult<usize, (usize, Job<usize>)> {
        let job = Box::new(move || {
            let some_time = time::Duration::from_millis(10);
            thread::sleep(some_time);
            res
        });
        tp.apply(job)
    }

    fn poll_n_check(job_id: usize, expected_res: usize, millis: u64, tp: &mut ThreadPool<usize>) {
        let polling_period = time::Duration::from_millis(millis);

        let now = Instant::now();
        let mut res = tp.result(job_id);
        while res.is_none() {
            thread::sleep(polling_period);
            println!("{} millis elapsed", now.elapsed().as_millis());
            res = tp.result(job_id);
        }
        assert_eq!(res, Some(expected_res));
    }

    for i in 0..20 {
        let job_id = apply_job(i, &mut tp)?;
        poll_n_check(job_id, i, i as u64, &mut tp);
    }

    assert_eq!(tp.result_queue.len(), 0);
    Ok(())
}
