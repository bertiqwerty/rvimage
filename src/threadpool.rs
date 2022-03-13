use crate::result::{RvError, RvResult};
use std::{
    fmt::Debug,
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{self, Instant},
};

type Job<T> = Box<dyn FnOnce() -> T + Send + 'static>;

fn poll<T, F: FnMut() -> Option<T>>(
    query_result: &mut F,
    interval_millis: u64,
    timeout_millis: Option<u128>,
) -> Option<T> {
    let interval = time::Duration::from_millis(interval_millis);
    let now = Instant::now();
    let mut time_out = 0;
    let predicate: fn(&Instant, u128) -> bool = match timeout_millis {
        Some(to) => {
            time_out = to;
            |now, to: u128| now.elapsed().as_millis() < to
        }
        None => |_, _| true,
    };
    let mut res = query_result();
    while res.is_none() && predicate(&now, time_out) {
        thread::sleep(interval);
        res = query_result();
    }
    res
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
            interval_millis: 10,
            timeout_millis: None,
        }
    }

    pub fn result(&mut self, job_id: usize) -> Option<T> {
        self.result_queue
            .extend(self.rx_from_pool.try_iter().flat_map(|received| received));
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
        let to = self.timeout_millis;
        poll(&mut || self.result(job_id), interv, to)
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
