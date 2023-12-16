use crate::result::{to_rv, RvError, RvResult};
use tracing::{error, info};

use std::{
    fmt::{self, Debug, Formatter},
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{self, Duration, Instant},
};

type Job<T> = Box<dyn FnOnce() -> T + Send + 'static>;

#[allow(dead_code)]
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

#[allow(dead_code)]
fn poll_timeout<T, F: FnMut() -> Option<T>>(
    query_result: &mut F,
    interval_millis: u64,
    timeout_millis: u128,
) -> Option<T> {
    let now = Instant::now();
    let predicate = move || now.elapsed().as_millis() < timeout_millis;
    poll(query_result, interval_millis, &predicate)
}

#[allow(dead_code)]
fn poll_until_result<T, F: FnMut() -> Option<T>>(
    query_result: &mut F,
    interval_millis: u64,
) -> Option<T> {
    let predicate = || true;
    poll(query_result, interval_millis, &predicate)
}

enum Message<J> {
    Terminate,
    NewJob(J),
}

pub fn result<T: Send + 'static>(
    rx_from_pool: &mut Receiver<(u128, T)>,
    result_queue: &mut Vec<(u128, T)>,
    job_id: u128,
) -> Option<T> {
    result_queue.extend(rx_from_pool.try_iter());
    let result = result_queue
        .iter()
        .enumerate()
        .find(|(_, (jid, _))| job_id == *jid);

    match result {
        None => None,
        Some((vec_idx, _)) => {
            let (_, v) = result_queue.remove(vec_idx);
            Some(v)
        }
    }
}

fn send_answer_message<T>(
    job_id: u128,
    f: Job<T>,
    tx_from_pool: &Sender<(u128, T)>,
    idx_thread: usize,
) -> Option<()> {
    let send_result = tx_from_pool.send((job_id, f()));
    match send_result {
        Ok(_) => {
            tracing::debug!("thread {idx_thread} send a result.");
            Some(())
        }
        Err(e) => {
            error!("thread {idx_thread} terminated. receiver gone.");
            error!("error: {e:?}");
            Some(())
        }
    }
}
type Tx2Tp<T> = Sender<Message<(u128, Job<T>)>>;
#[allow(dead_code)]
pub struct ThreadPool<T: Send + 'static> {
    txs_to_pool: Vec<Tx2Tp<T>>,
    rx_from_pool: Receiver<(u128, T)>,
    next_thread: usize,
    job_id: u128,
    result_queue: Vec<(u128, T)>,
    interval_millis: u64,
    timeout_millis: Option<u128>,
}
impl<T: Send + 'static> ThreadPool<T> {
    pub fn new(n_threads: usize) -> Self {
        let mut txs_to_pool = Vec::with_capacity(n_threads);
        let (tx_from_pool, rx_from_pool) = mpsc::channel();
        for idx_thread in 0..n_threads {
            let (tx_to_pool, rx_to_pool) = mpsc::channel();
            txs_to_pool.push(tx_to_pool);
            let tx = tx_from_pool.clone();
            let thread = move || -> RvResult<()> {
                info!("spawning thread {idx_thread}");
                loop {
                    let received_msg = rx_to_pool.recv().map_err(to_rv)?;
                    match received_msg {
                        Message::Terminate => {
                            info!("shut down thread {idx_thread}");
                            return Ok(());
                        }
                        Message::NewJob((i, f)) => {
                            send_answer_message(i, f, &tx, idx_thread);
                        }
                    }
                }
            };
            thread::spawn(thread);
        }
        ThreadPool {
            txs_to_pool,
            rx_from_pool,
            next_thread: 0usize,
            job_id: 0u128,
            result_queue: vec![],
            interval_millis: 10,
            timeout_millis: None,
        }
    }

    pub fn result(&mut self, job_id: u128) -> Option<T> {
        result(&mut self.rx_from_pool, &mut self.result_queue, job_id)
    }
    fn apply_id(&mut self, f: Job<T>, job_id: u128) -> RvResult<()> {
        if self.next_thread == self.txs_to_pool.len() {
            self.next_thread = 0;
        }
        tracing::debug!("sending id {job_id:?}");
        self.txs_to_pool[self.next_thread]
            .send(Message::NewJob((job_id, f)))
            .map_err(|e| RvError::new(&e.to_string()))?;
        self.next_thread += 1;

        Ok(())
    }
    pub fn apply(&mut self, f: Job<T>) -> RvResult<u128> {
        self.apply_id(f, self.job_id)?;
        self.job_id = if self.job_id < u128::MAX {
            self.job_id + 1
        } else {
            0
        };
        Ok(self.job_id - 1)
    }
    #[allow(dead_code)]
    pub fn poll(&mut self, job_id: u128) -> Option<T> {
        let interv = self.interval_millis;
        let timeout = self.timeout_millis;
        let query_result = &mut || self.result(job_id);
        match timeout {
            Some(to) => poll_timeout(query_result, interv, to),
            None => poll_until_result(query_result, interv),
        }
    }
}
impl<T> Default for ThreadPool<T>
where
    T: Send + 'static,
{
    fn default() -> Self {
        ThreadPool::new(1)
    }
}

fn terminate_all_threads<T: Send + 'static>(tp: &ThreadPool<T>) -> RvResult<()> {
    for tx in &tp.txs_to_pool {
        tx.send(Message::Terminate).map_err(to_rv)?;
    }
    Ok(())
}

impl<T: Send + 'static> Drop for ThreadPool<T> {
    fn drop(&mut self) {
        match terminate_all_threads(self) {
            Ok(_) => (),
            Err(e) => {
                error!("error when dropping threadpool, {e:?}");
            }
        }
    }
}

fn update_prio<T: Send + 'static>(
    job_id_to_change: u128,
    new_prio: Option<usize>,
    jobs_queue: &mut Vec<JobQueued<T>>,
) {
    let change_idx_opt = jobs_queue.iter().position(|j| j.job_id == job_id_to_change);
    if let Some(change_idx) = change_idx_opt {
        match new_prio {
            None => {
                jobs_queue.remove(change_idx);
            }
            Some(prio) => {
                jobs_queue[change_idx].prio = prio;
            }
        }
    }
}
// submit new job if we have less jobs than threads and the respective delays have
// passed
fn submit_job<T: Send + 'static>(
    n_threads: usize,
    jobs_running: &mut Vec<u128>,
    jobs_queue: &mut Vec<JobQueued<T>>,
    tp: &mut ThreadPool<T>,
) -> RvResult<()> {
    if n_threads > jobs_running.len() {
        let n_jobs = jobs_queue.len();
        // look for the job with the highest priority and execute that
        if let Some((max_prio_idx, _)) = jobs_queue
            .iter()
            .enumerate()
            .filter(|(_, j)| j.started.elapsed().as_millis() >= j.delay_ms)
            .max_by_key(|(_, j)| j.prio)
        {
            jobs_queue.swap(max_prio_idx, n_jobs - 1);
            let job = &jobs_queue[n_jobs - 1];
            let job_id = job.job_id;
            jobs_running.push(job_id);
            tp.apply_id(jobs_queue.pop().unwrap().f, job_id)?;
        }
    }
    Ok(())
}
struct JobQueued<T: Send + 'static> {
    f: Job<T>,
    prio: usize,
    delay_ms: u128,
    job_id: u128,
    started: Instant,
}
impl<T: Send + 'static> Debug for JobQueued<T> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(
            f,
            "job id: {}, prio: {},  delay_ms{}, started: {:?}",
            self.job_id, self.prio, self.delay_ms, self.started
        )
    }
}
pub struct ThreadPoolQueued<T: Send + 'static> {
    job_id: u128,
    tx_job_to_pool: Sender<Message<JobQueued<T>>>,
    rx_job_result_from_pool: Receiver<(u128, T)>,
    tx_prio_to_pool: Sender<(u128, Option<usize>)>, // send job id and new priority
    result_queue: Vec<(u128, T)>,
}
impl<T: Send + 'static> ThreadPoolQueued<T> {
    pub fn new(n_threads: usize) -> Self {
        let (tx_job_to_pool, rx_job_to_pool) = mpsc::channel();
        let (tx_job_result_from_pool, rx_job_result_from_pool) = mpsc::channel();
        let (tx_prio_to_pool, rx_prio_to_pool) = mpsc::channel();
        let thread = move || -> RvResult<()> {
            let mut tp: ThreadPool<T> = ThreadPool::new(n_threads);
            let mut jobs_queue = Vec::new();
            let mut jobs_running = Vec::new();
            loop {
                // update job queue
                for msg in rx_job_to_pool.try_iter() {
                    match msg {
                        Message::Terminate => {
                            return Ok(());
                        }
                        Message::<JobQueued<T>>::NewJob(job_queued) => {
                            jobs_queue.push(job_queued);
                        }
                    }
                }
                // update priorities
                for (job_id_to_change, new_prio) in rx_prio_to_pool.try_iter() {
                    update_prio(job_id_to_change, new_prio, &mut jobs_queue);
                }

                submit_job(n_threads, &mut jobs_running, &mut jobs_queue, &mut tp)?;

                // send results of finished jobs
                let results = jobs_running
                    .iter()
                    .map(|job_id| tp.result(*job_id))
                    .collect::<Vec<_>>();
                let mut finished_job_ids: Vec<u128> = Vec::new();
                jobs_running = jobs_running
                    .into_iter()
                    .enumerate()
                    .filter(|(i, job_id)| match results[*i] {
                        None => true,
                        Some(_) => {
                            finished_job_ids.push(*job_id);
                            false
                        }
                    })
                    .map(|(_, job_id)| job_id)
                    .collect::<Vec<_>>();
                for (i, res) in results.into_iter().flatten().enumerate() {
                    tx_job_result_from_pool
                        .send((finished_job_ids[i], res))
                        .map_err(to_rv)?;
                }
                thread::sleep(Duration::from_millis(1));
            }
        };
        thread::spawn(thread);
        ThreadPoolQueued {
            job_id: 0,
            tx_job_to_pool,
            rx_job_result_from_pool,
            tx_prio_to_pool,
            result_queue: vec![],
        }
    }
    pub fn apply(&mut self, job: Job<T>, prio: usize, delay_ms: u128) -> RvResult<u128> {
        self.tx_job_to_pool
            .send(Message::NewJob(JobQueued {
                f: job,
                prio,
                delay_ms,
                job_id: self.job_id,
                started: Instant::now(),
            }))
            .map_err(to_rv)?;
        self.job_id = if self.job_id < u128::MAX {
            self.job_id + 1
        } else {
            0
        };
        Ok(self.job_id - 1)
    }
    /// Updates the priority if not yet submitted, None means cancel
    pub fn update_prio(&self, job_id: u128, new_prio: Option<usize>) -> RvResult<()> {
        self.tx_prio_to_pool
            .send((job_id, new_prio))
            .map_err(to_rv)?;
        Ok(())
    }
    pub fn result(&mut self, job_id: u128) -> Option<T> {
        result(
            &mut self.rx_job_result_from_pool,
            &mut self.result_queue,
            job_id,
        )
    }
}
impl<T: Send + 'static> Drop for ThreadPoolQueued<T> {
    fn drop(&mut self) {
        match self.tx_job_to_pool.send(Message::Terminate) {
            Ok(_) => (),
            Err(e) => {
                error!("error when dropping ThreadPoolQueued, {e:?}");
            }
        }
    }
}

#[cfg(test)]
fn make_test_job_sleep(res: i32, sleep_ms: u64) -> Job<i32> {
    Box::new(move || {
        thread::sleep(Duration::from_millis(sleep_ms));
        res
    })
}
#[cfg(test)]
fn make_test_job(res: i32) -> Job<i32> {
    make_test_job_sleep(res, 20)
}
#[cfg(test)]
fn make_test_queue() -> Vec<JobQueued<i32>> {
    let mut jobs_queue = vec![];
    for i in 0..20 {
        jobs_queue.push(JobQueued {
            prio: 1,
            job_id: i,
            f: make_test_job(i as i32),
            started: Instant::now(),
            delay_ms: 0,
        });
    }
    jobs_queue
}
#[test]
fn test_prio() -> RvResult<()> {
    let mut jobs_queue = make_test_queue();
    let mut test_update_prio = |job_id_to_change, new_prio| -> RvResult<()> {
        println!("setting {} to {:?}", job_id_to_change, new_prio);
        update_prio(job_id_to_change, new_prio, &mut jobs_queue);
        assert_eq!(
            jobs_queue
                .iter()
                .find(|j| j.job_id == job_id_to_change)
                .map(|j| j.prio),
            new_prio
        );
        Ok(())
    };
    test_update_prio(0, Some(234))?;
    test_update_prio(1, None)?;
    test_update_prio(13, Some(577))?;
    Ok(())
}
#[test]
fn test_submit() -> RvResult<()> {
    let mut jobs_queue = make_test_queue();
    let n_threads = 2;
    let mut tp = ThreadPool::<i32>::new(n_threads);
    let mut jobs_running = vec![];
    update_prio(0, Some(234), &mut jobs_queue);
    update_prio(1, None, &mut jobs_queue);
    update_prio(13, Some(577), &mut jobs_queue);
    submit_job(n_threads, &mut jobs_running, &mut vec![], &mut tp)?;
    assert_eq!(jobs_running.len(), 0);
    assert!(jobs_queue.iter().find(|j| j.job_id == 13).is_some());
    submit_job(n_threads, &mut jobs_running, &mut jobs_queue, &mut tp)?;
    assert!(jobs_running.iter().find(|j| **j == 13).is_some());
    assert!(jobs_queue.iter().find(|j| j.job_id == 13).is_none());
    assert!(jobs_queue.iter().find(|j| j.job_id == 0).is_some());
    submit_job(n_threads, &mut jobs_running, &mut jobs_queue, &mut tp)?;
    assert!(jobs_running.iter().find(|j| **j == 0).is_some());
    assert!(jobs_queue.iter().find(|j| j.job_id == 0).is_none());
    let n_threads = 20;
    let mut tp = ThreadPool::<i32>::new(n_threads);
    let mut new_queue = make_test_queue();
    let mut jobs_running = vec![];
    new_queue[0].delay_ms = 1000;
    for _ in 0..20 {
        submit_job(n_threads, &mut jobs_running, &mut new_queue, &mut tp)?;
    }
    println!("{:?}", jobs_running);
    assert!(jobs_running.iter().find(|jid| **jid == 0).is_none());
    assert!(jobs_running.iter().find(|jid| **jid == 1).is_some());
    Ok(())
}
#[test]
fn test_tp_queued() -> RvResult<()> {
    let mut tpq = ThreadPoolQueued::new(1);
    tpq.apply(make_test_job(17), 0, 0)?;
    assert!(tpq.result(0).is_none());
    thread::sleep(Duration::from_millis(50));
    assert_eq!(tpq.result(0), Some(17));
    assert_eq!(tpq.result(0), None);
    let jid1 = tpq.apply(make_test_job(11), 0, 0)?;
    let jid2 = tpq.apply(make_test_job(12), 0, 0)?;
    println!("jid1 {}, jid2, {}", jid1, jid2);
    thread::sleep(Duration::from_millis(150));
    assert_eq!(tpq.result(jid1), Some(11));
    assert_eq!(tpq.result(jid2), Some(12));
    Ok(())
}
#[test]
fn test_tp_update_prio() -> RvResult<()> {
    let mut tpq = ThreadPoolQueued::new(1);
    let jid = tpq.apply(make_test_job_sleep(5, 20), 0, 10)?;
    tpq.update_prio(jid, None)?;
    thread::sleep(Duration::from_millis(140));
    assert_eq!(tpq.result(jid), None);
    let jid_1 = tpq.apply(make_test_job_sleep(5, 20), 0, 30)?;
    let jid_2 = tpq.apply(make_test_job_sleep(10, 20), 1, 30)?;
    tpq.update_prio(jid_1, Some(10))?;
    for _ in 30..250 {
        thread::sleep(Duration::from_millis(1));
        let r1 = tpq.result(jid_1);
        let r2 = tpq.result(jid_2);
        assert!(!(r1.is_none() && r2.is_some()));
        if r1.is_some() {
            break;
        }
    }
    thread::sleep(Duration::from_millis(300));
    assert_eq!(tpq.result(jid_2), Some(10));
    Ok(())
}
#[test]
fn test_tp_delay() -> RvResult<()> {
    let mut tpq = ThreadPoolQueued::new(1);
    let ref_lo = 47;
    let ref_hi = 23;
    let j_lo = make_test_job_sleep(ref_lo, 200);
    let j_hi = make_test_job_sleep(ref_hi, 200);
    let jid_hi = tpq.apply(j_hi, 50, 350)?;
    let jid_lo = tpq.apply(j_lo, 49, 0)?;
    thread::sleep(Duration::from_millis(300));
    let res_hi = tpq.result(jid_hi);
    let res_lo = tpq.result(jid_lo);
    assert_eq!(res_hi, None);
    assert_eq!(res_lo, Some(ref_lo));
    thread::sleep(Duration::from_millis(300));
    let res_hi = tpq.result(jid_hi);
    assert_eq!(res_hi, Some(ref_hi));
    Ok(())
}
#[test]
fn test_tp_prio() -> RvResult<()> {
    let mut tpq = ThreadPoolQueued::new(1);
    let ref_lo = 47;
    let ref_hi = 23;
    let j_lo = make_test_job_sleep(ref_lo, 200);
    let j_hi = make_test_job_sleep(ref_hi, 200);
    let jid_hi = tpq.apply(j_hi, 50, 0)?;
    let jid_lo = tpq.apply(j_lo, 49, 0)?;
    thread::sleep(Duration::from_millis(300));
    let res_hi = tpq.result(jid_hi);
    let res_lo = tpq.result(jid_lo);
    assert_eq!(res_hi, Some(ref_hi));
    assert_eq!(res_lo, None);
    thread::sleep(Duration::from_millis(300));
    let res_lo = tpq.result(jid_lo);
    assert_eq!(res_lo, Some(ref_lo));
    Ok(())
}
#[test]
fn test_tp() -> RvResult<()> {
    let mut tp = ThreadPool::new(4);

    fn apply_job(res: usize, tp: &mut ThreadPool<usize>) -> RvResult<u128> {
        let job = Box::new(move || {
            let some_time = time::Duration::from_millis(10);
            thread::sleep(some_time);
            res
        });
        tp.apply(job)
    }

    fn poll_n_check(job_id: u128, expected_res: usize, tp: &mut ThreadPool<usize>) {
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
