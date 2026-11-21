use super::*;
use std::{
    path::Path,
    sync::{Condvar, atomic::AtomicUsize},
    time::{Duration, Instant},
};
struct Loader {
    calls: AtomicUsize,
    gate: Arc<(Mutex<bool>, Condvar)>,
    entered: SyncSender<()>,
}
impl ImageLoader for Loader {
    fn load(&self, path: &Path, e: Extent) -> PortResult<Vec<u8>> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        let _ = self.entered.try_send(());
        let lock = self.gate.0.lock().unwrap();
        let _ = self
            .gate
            .1
            .wait_timeout_while(lock, Duration::from_secs(2), |open| !*open)
            .unwrap();
        let color = if path == Path::new("first") { 1 } else { 2 };
        Ok(vec![color; (e.width * e.height * 4) as usize])
    }
}
fn job(ticket: u64, key: Arc<AtomicU64>, path: &str) -> Job {
    Job {
        ticket,
        current: key,
        source: Arc::new(Source::File {
            path: path.into(),
            asset: vjay_domain::node::AssetId(0),
        }),
        extent: Extent::new(2, 2).unwrap(),
    }
}
#[test]
fn requests_remain_nonblocking_stale_results_are_discarded_and_cached_pixels_are_reused() {
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let (entered, started) = mpsc::sync_channel(8);
    let loader = Arc::new(Loader {
        calls: AtomicUsize::new(0),
        gate: gate.clone(),
        entered,
    });
    let pool = Pool::new(Some(loader.clone()), None).unwrap();
    let current = Arc::new(AtomicU64::new(1));
    assert!(
        pool.submit(job(1, current.clone(), "first"))
            .unwrap()
            .is_none()
    );
    started.recv_timeout(Duration::from_secs(1)).unwrap();
    let begin = Instant::now();
    assert!(pool.poll().unwrap().is_none());
    assert!(begin.elapsed() < Duration::from_millis(50));
    current.store(2, Ordering::Release);
    assert!(
        pool.submit(job(2, current.clone(), "second"))
            .unwrap()
            .is_none()
    );
    *gate.0.lock().unwrap() = true;
    gate.1.notify_all();
    let deadline = Instant::now() + Duration::from_secs(3);
    let done = loop {
        if let Some(done) = pool.wait().unwrap() {
            break done;
        }
        assert!(Instant::now() < deadline);
    };
    assert_eq!(done.ticket, 2);
    assert!(done.pixels.unwrap().iter().all(|&v| v == 2));
    let before = loader.calls.load(Ordering::Relaxed);
    current.store(3, Ordering::Release);
    pool.submit(job(3, current, "second")).unwrap();
    let done = loop {
        if let Some(done) = pool.wait().unwrap() {
            break done;
        }
        assert!(Instant::now() < deadline);
    };
    assert_eq!(done.ticket, 3);
    assert_eq!(loader.calls.load(Ordering::Relaxed), before);
}
#[test]
fn saturated_work_is_bounded_and_teardown_does_not_join_a_blocked_loader() {
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let (entered, started) = mpsc::sync_channel(8);
    let loader = Arc::new(Loader {
        calls: AtomicUsize::new(0),
        gate: gate.clone(),
        entered,
    });
    let pool = Pool::new(Some(loader), None).unwrap();
    pool.submit(job(1, Arc::new(AtomicU64::new(1)), "first"))
        .unwrap();
    pool.submit(job(2, Arc::new(AtomicU64::new(2)), "second"))
        .unwrap();
    for _ in 0..2 {
        started.recv_timeout(Duration::from_secs(1)).unwrap();
    }
    for ticket in 3..19 {
        assert!(
            pool.submit(job(ticket, Arc::new(AtomicU64::new(ticket)), "queued"))
                .unwrap()
                .is_none()
        );
    }
    assert!(
        pool.submit(job(19, Arc::new(AtomicU64::new(19)), "overflow"))
            .unwrap()
            .is_some()
    );
    let begin = Instant::now();
    drop(pool);
    assert!(begin.elapsed() < Duration::from_millis(100));
    *gate.0.lock().unwrap() = true;
    gate.1.notify_all();
}
