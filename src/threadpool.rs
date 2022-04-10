use crate::result::{RvError, RvResult};
use std::{
    fmt::Debug,
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{self, Instant},
};

type Job<T> = Box<dyn FnOnce() -> T + Send + 'static>;

fn poll<T, F1: FnMut() -> Option<T>, F2: Fn() -> bool>(
    query_result: &mut F1,
    interval_millis: u64,
    predicate: &F2,
) -> Option<T> {
    let interval = time::Duration::from_millis(interval_millis);
    let mut res = query_result();
    while res.is_none() && predicate() {
        thread::sleep(interval);
        res = query_result();
    }
    res
}

fn poll_timeout<T, F1: FnMut() -> Option<T>>(
    query_result: &mut F1,
    interval_millis: u64,
    timeout_millis: u128,
) -> Option<T> {
    let now = Instant::now();
    let predicate = move || now.elapsed().as_millis() < timeout_millis;
    poll(query_result, interval_millis, &predicate)
}

fn poll_until_result<T, F1: FnMut() -> Option<T>>(
    query_result: &mut F1,
    interval_millis: u64,
) -> Option<T> {
    let predicate = || true;
    poll(query_result, interval_millis, &predicate)
}

pub struct ThreadPool<T: Debug + Clone + Send + 'static> {
    txs_to_pool: Vec<Sender<(usize, Job<T>)>>,
    rx_from_pool: Receiver<RvResult<(usize, T)>>,
    next_thread: usize,
    job_id: usize,
    result_queue: Vec<(usize, T)>,
    interval_millis: u64,
    timeout_millis: Option<u128>,
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
                    let send_result = tx.send(
                        rx_to_pool
                            .recv()
                            .map(|(i, f): (usize, Job<T>)| (i, f()))
                            .map_err(|e| RvError::new(&e.to_string())),
                    );
                    match send_result {
                        Ok(_) => {
                            println!("thread {} send a result.", idx_thread);
                        }
                        Err(e) => {
                            println!("thread {} terminated. receiver gone.", idx_thread);
                            println!("error: {:?}", e);
                            return;
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
            interval_millis: 10,
            timeout_millis: None,
        }
    }

    pub fn result(&mut self, job_id: usize) -> Option<T> {
        self.result_queue
            .extend(self.rx_from_pool.try_iter().flatten());
        let result = self
            .result_queue
            .iter()
            .enumerate()
            .find(|(_, (jid, _))| job_id == *jid);

        match result {
            None => None,
            Some((vec_idx, _)) => {
                let (_, v) = self.result_queue.remove(vec_idx);
                Some(v)
            }
        }
    }

    pub fn apply(&mut self, f: Job<T>) -> RvResult<usize> {
        // paranoia check
        self.job_id = if self.job_id < usize::MAX {
            self.job_id + 1
        } else {
            0
        };
        if self.next_thread == self.txs_to_pool.len() {
            self.next_thread = 0;
        }
        println!("sending id {:?}", self.job_id);
        self.txs_to_pool[self.next_thread]
            .send((self.job_id, f))
            .map_err(|e| RvError::new(&e.to_string()))?;
        self.next_thread += 1;

        Ok(self.job_id)
    }

    pub fn poll(&mut self, job_id: usize) -> Option<T> {
        let interv = self.interval_millis;
        let timeout = self.timeout_millis;
        let query_result = &mut || self.result(job_id);
        match timeout {
            Some(to) => poll_timeout(query_result, interv, to),
            None => poll_until_result(query_result, interv)
        }
    }
}

#[test]
fn test() -> RvResult<()> {
    let mut tp = ThreadPool::new(4);

    fn apply_job(res: usize, tp: &mut ThreadPool<usize>) -> RvResult<usize> {
        let job = Box::new(move || {
            let some_time = time::Duration::from_millis(10);
            thread::sleep(some_time);
            res
        });
        tp.apply(job)
    }

    fn poll_n_check(job_id: usize, expected_res: usize, tp: &mut ThreadPool<usize>) {
        let res = tp.poll(job_id);
        assert_eq!(res, Some(expected_res));
    }

    for i in 0..20 {
        let job_id = apply_job(i, &mut tp)?;
        poll_n_check(job_id, i, &mut tp);
    }

    assert_eq!(tp.result_queue.len(), 0);
    Ok(())
}
