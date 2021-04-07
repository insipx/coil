// Copyright 2018-2019 Parity Technologies (UK) Ltd.
// This file is part of coil.

// coil is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// coil is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with coil.  If not, see <http://www.gnu.org/licenses/>.

use crate::job::Job;
use crate::{db, error::*, registry::Registry};
use channel::Sender;
use futures::executor::block_on;
use sqlx::PgPool;
use sqlx::Postgres;
use std::any::Any;
use std::panic::{catch_unwind, AssertUnwindSafe, PanicInfo, RefUnwindSafe, UnwindSafe};
use std::sync::Arc;
use std::time::Duration;

/// Builder pattern struct for the Runner
pub struct Builder<Env> {
    environment: Env,
    num_threads: Option<usize>,
    pg_pool: sqlx::PgPool,
    max_tasks: Option<usize>,
    registry: Registry<Env>,
    on_finish: Option<Arc<dyn Fn(i64) + Send + Sync + 'static>>,
    /// Amount of time to wait until job is deemed a failure
    timeout: Option<Duration>,
}

impl<Env: 'static> Builder<Env> {
    /// Instantiate a new instance of the Builder
    pub fn new(environment: Env, pg_pool: sqlx::PgPool) -> Self {
        Self {
            environment,
            pg_pool,
            max_tasks: None,
            num_threads: None,
            registry: Registry::load(),
            on_finish: None,
            timeout: None,
        }
    }

    ///  Register a job that hasn't or can't be registered by invoking the `register_job!` macro
    ///
    /// Jobs that include generics must use this function in order to be registered with a runner.
    /// Jobs must be registered with every generic that is used.
    /// Jobs are available in the format `my_function_name::Job`.
    ///
    ///  # Example
    ///  ```ignore
    ///  RunnerBuilder::new(env, conn)
    ///      .register_job::<resize_image::Job<String>>()
    ///  ```
    ///  Different jobs must be registered with different generics if they exist.
    ///
    ///  ```ignore
    ///  RunnerBuilder::new((), conn)
    ///     .register_job::<resize_image::Job<String>>()
    ///     .register_job::<resize_image::Job<u32>>()
    ///     .register_job::<resize_image::Job<MyStruct>()
    ///  ```
    ///
    pub fn register_job<T: Job + 'static + Send>(mut self) -> Self {
        self.registry.register_job::<T>();
        self
    }

    /// specify the amount of threads to run the threadpool with
    pub fn num_threads(mut self, threads: usize) -> Self {
        self.num_threads = Some(threads);
        self
    }

    /// Specify the maximum tasks  to queue in the threadpool at any given time
    pub fn max_tasks(mut self, max_tasks: usize) -> Self {
        self.max_tasks = Some(max_tasks);
        self
    }

    /// Provide a hook that runs after a job has finished and all destructors have run
    /// the `on_finish` closure accepts the job ID that finished as an argument
    pub fn on_finish(mut self, on_finish: impl Fn(i64) + Send + Sync + 'static) -> Self {
        self.on_finish = Some(Arc::new(on_finish));
        self
    }

    /// Set a timeout in seconds.
    /// This timeout is the maximum amount of time coil will wait for a job to begin
    /// before returning an error.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Build the runner
    pub fn build(self) -> Result<Runner<Env>, Error> {
        let threadpool = if let Some(t) = self.num_threads {
            rayon::ThreadPoolBuilder::new()
                .num_threads(t)
                .thread_name(|i| format!("coil-{}", i))
        } else {
            rayon::ThreadPoolBuilder::new().thread_name(|i| format!("coil-{}", i))
        };
        let threadpool = threadpool.build()?;

        let max_tasks = self
            .max_tasks
            .unwrap_or_else(|| threadpool.current_num_threads());
        let timeout = self
            .timeout
            .unwrap_or_else(|| std::time::Duration::from_secs(5));
        Ok(Runner {
            threadpool,
            pg_pool: self.pg_pool,
            environment: Arc::new(self.environment),
            registry: Arc::new(self.registry),
            max_tasks,
            on_finish: self.on_finish,
            timeout,
        })
    }
}

/// Runner for background tasks.
/// Synchronous tasks are run in a threadpool.
pub struct Runner<Env> {
    threadpool: rayon::ThreadPool,
    pg_pool: PgPool,
    environment: Arc<Env>,
    registry: Arc<Registry<Env>>,
    /// maximum number of tasks to run at any one time
    max_tasks: usize,
    on_finish: Option<Arc<dyn Fn(i64) + Send + Sync + 'static>>,
    timeout: Duration,
}

///
pub enum Event {
    /// Queues are currently working
    Working,
    /// No more jobs available in queue
    NoJobAvailable,
    /// An error occurred loading the job from the database
    ErrorLoadingJob(sqlx::Error),
    /// Test for waiting on dummy tasks
    #[doc(hidden)]
    #[cfg(any(test, feature = "test_components"))]
    Dummy,
}

type TxJobPair = Option<(
    sqlx::Transaction<'static, sqlx::Postgres>,
    db::BackgroundJob,
)>;

// Methods which don't require `RefUnwindSafe`
impl<Env: 'static> Runner<Env> {
    /// Build the builder for `Runner`
    pub fn builder(env: Env, conn: &sqlx::PgPool) -> Builder<Env> {
        Builder::new(env, conn.clone())
    }

    /// Get a Pool Connection from the pool that the runner is using.
    pub async fn connection(&self) -> Result<sqlx::pool::PoolConnection<sqlx::Postgres>, Error> {
        let conn = self.pg_pool.acquire().await?;
        Ok(conn)
    }

    /// Get the connection pool that the runner is using
    pub fn connection_pool(&self) -> sqlx::PgPool {
        self.pg_pool.clone()
    }
}

impl<Env: Send + Sync + RefUnwindSafe + 'static> Runner<Env> {
    /// Runs all the pending tasks in a loop
    /// Returns how many tasks are running as a result
    pub fn run_pending_tasks<F>(&self) -> Result<usize, FetchError> {
        let (tx, rx) = channel::bounded(self.max_tasks);

        let mut pending_messages = 0;
        let mut queued = 0;
        loop {
            let jobs_to_queue = if pending_messages == 0 {
                self.max_tasks
            } else {
                self.max_tasks - pending_messages
            };

            for _ in 0..jobs_to_queue {
                self.run_single_sync_job(tx.clone())
            }

            pending_messages += jobs_to_queue;
            match rx.recv_timeout(self.timeout) {
                Ok(Event::Working) => {
                    pending_messages -= 1;
                    queued += 1;
                }
                Ok(Event::NoJobAvailable) => return Ok(queued),
                Ok(Event::ErrorLoadingJob(e)) => return Err(FetchError::FailedLoadingJob(e)),
                Err(_) => return Err(FetchError::NoMessage.into()),
                _ => return Ok(queued),
            }
        }
    }

    fn run_single_sync_job(&self, tx: Sender<Event>) {
        let env = Arc::clone(&self.environment);
        let registry = Arc::clone(&self.registry);
        let pg_pool = AssertUnwindSafe(self.pg_pool.clone());

        self.get_single_sync_job(tx, move |job| {
            let perform_fn = registry
                .get(&job.job_type)
                .ok_or_else(|| PerformError::from(format!("Unknown job type {}", job.job_type)))?;
            perform_fn.perform_sync(job.data, &env, &pg_pool)
        });
    }

    fn get_single_sync_job<F>(&self, tx: Sender<Event>, fun: F)
    where
        F: FnOnce(db::BackgroundJob) -> Result<(), PerformError> + Send + UnwindSafe + 'static,
    {
        let pg_pool = self.pg_pool.clone();
        let finish_hook = self.on_finish.clone();
        self.threadpool.spawn_fifo(move || {
            let res = move || -> Result<(), PerformError> {
                let (transaction, job) =
                    if let Some((t, j)) = block_on(Self::get_next_job(tx, &pg_pool)) {
                        (t, j)
                    } else {
                        return Ok(());
                    };
                let job_id = job.id;
                let result = catch_unwind(|| fun(job))
                    .map_err(|e| try_to_extract_panic_info(&e))
                    .and_then(|r| r);
                block_on(Self::finish_work(result, transaction, job_id, finish_hook));
                Ok(())
            };

            match res() {
                Ok(_) => {}
                Err(e) => {
                    panic!("Failed to update job: {:?}", e);
                }
            }
        });
    }

    /// returns a transaction/job pair for the next Job
    async fn get_next_job(tx: Sender<Event>, pg_pool: &PgPool) -> TxJobPair {
        let mut transaction = match pg_pool.begin().await {
            Ok(t) => t,
            Err(e) => {
                let _ = tx.send(Event::ErrorLoadingJob(e));
                return None;
            }
        };

        let job = match db::find_next_unlocked_job(&mut transaction).await {
            Ok(Some(j)) => {
                let _ = tx.send(Event::Working);
                j
            }
            Ok(None) => {
                let _ = tx.send(Event::NoJobAvailable);
                return None;
            }
            Err(e) => {
                let _ = tx.send(Event::ErrorLoadingJob(e));
                return None;
            }
        };
        Some((transaction, job))
    }

    async fn finish_work(
        res: Result<(), PerformError>,
        mut trx: sqlx::Transaction<'static, Postgres>,
        job_id: i64,
        on_finish: Option<Arc<dyn Fn(i64) + Send + Sync + 'static>>,
    ) {
        match res {
            Ok(_) => {
                db::delete_successful_job(&mut trx, job_id)
                    .await
                    .map_err(|e| panic!("Failed to delete job: {:?}", e))
                    .expect("Panic is mapped");
            }
            Err(e) => {
                // TODO: Fix killing the execution
                // eprintln!("Job {} failed to run: {}", job_id, e);
                db::update_failed_job(&mut trx, job_id)
                    .await
                    .expect(&format!("failed to update failed job: {:?}", e));
            }
        }

        trx.commit().await.expect("Failed to commit transaction");
        if let Some(f) = on_finish {
            f(job_id)
        }
    }
}

fn try_to_extract_panic_info(info: &(dyn Any + Send + 'static)) -> PerformError {
    if let Some(x) = info.downcast_ref::<PanicInfo>() {
        format!("job panicked: {}", x).into()
    } else if let Some(x) = info.downcast_ref::<&'static str>() {
        format!("job panicked: {}", x).into()
    } else if let Some(x) = info.downcast_ref::<String>() {
        format!("job panicked: {}", x).into()
    } else {
        "job panicked".into()
    }
}

#[cfg(any(test, feature = "test_components"))]
impl<Env: Send + Sync + RefUnwindSafe + 'static> Runner<Env> {
    /// Wait for tasks to finish based on timeout
    /// this is mostly used for internal tests
    async fn wait_for_all_tasks(&self, rx: channel::Receiver<Event>, pending: usize) {
        use futures::{FutureExt, StreamExt};

        let mut dummy_tasks = pending;
        let mut rx = rx.into_stream();
        while dummy_tasks > 0 {
            let timeout = timer::Delay::new(self.timeout);
            futures::select! {
                msg = rx.next() => match msg {
                    // Some(Event::Working) => continue,
                    Some(Event::NoJobAvailable) => break,
                    Some(Event::Dummy) => dummy_tasks -= 1,
                    _ => (),
                },
                _ = timeout.fuse() => {
                    log::warn!("TASK WAIT TIMED OUT");
                    break
                },
            };
        }
    }

    /// Check for any jobs that may have failed
    pub async fn check_for_failed_jobs(
        &self,
        rx: channel::Receiver<Event>,
        pending: usize,
    ) -> Result<(), FailedJobsError> {
        self.wait_for_all_tasks(rx, pending).await;
        let num_failed = db::failed_job_count(&self.pg_pool).await.unwrap();
        if num_failed == 0 {
            Ok(())
        } else {
            Err(FailedJobsError::JobsFailed(num_failed))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use once_cell::sync::Lazy;
    use std::panic::AssertUnwindSafe;
    use std::sync::{Arc, Barrier, Mutex, MutexGuard};

    static TEST_MUTEX: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

    struct TestGuard<'a>(MutexGuard<'a, ()>);
    impl<'a> TestGuard<'a> {
        fn lock() -> Self {
            TestGuard(TEST_MUTEX.lock().unwrap())
        }
    }

    impl<'a> Drop for TestGuard<'a> {
        fn drop(&mut self) {
            smol::block_on(async move {
                sqlx::query("TRUNCATE TABLE _background_tasks")
                    .execute(&mut runner().connection().await.unwrap())
                    .await
                    .unwrap()
            });
        }
    }

    fn runner() -> Runner<()> {
        let database_url =
            dotenv::var("DATABASE_URL").expect("DATABASE_URL must be set to run tests");
        let pool = smol::block_on(sqlx::PgPool::connect(database_url.as_str())).unwrap();
        crate::Runner::builder((), &pool)
            .num_threads(2)
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap()
    }

    fn create_dummy_job(runner: &Runner<()>) -> i64 {
        let data = rmp_serde::to_vec(vec![0].as_slice()).unwrap();
        smol::block_on(async move {
            let mut conn = runner.connection().await.unwrap();
            let _rec = sqlx::query(
                "INSERT INTO _background_tasks (job_type, data)
                VALUES ($1, $2)
                RETURNING (id, job_type, data)",
            )
            .bind("Foo")
            .bind(data)
            .fetch_one(&mut conn)
            .await
            .unwrap();

            sqlx::query_as::<_, (i64,)>(
                "SELECT currval(pg_get_serial_sequence('_background_tasks', 'id'))",
            )
            .fetch_one(&mut conn)
            .await
            .unwrap()
            .0
        })
    }

    async fn get_job_count(conn: impl sqlx::Executor<'_, Database = sqlx::Postgres>) -> i64 {
        sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM _background_tasks")
            .fetch_one(conn)
            .await
            .unwrap()
            .0
    }

    #[test]
    fn sync_jobs_are_locked_when_fetched() {
        crate::initialize();
        let _guard = TestGuard::lock();
        let mut runner = runner();
        let first_job_id = create_dummy_job(&runner);
        let second_job_id = create_dummy_job(&runner);
        let fetch_barrier = Arc::new(AssertUnwindSafe(Barrier::new(2)));
        let fetch_barrier2 = fetch_barrier.clone();
        let return_barrier = Arc::new(AssertUnwindSafe(Barrier::new(2)));
        let return_barrier2 = return_barrier.clone();

        let (tx, rx) = channel::bounded(3);
        let tx0 = tx.clone();
        runner.on_finish = Some(Arc::new(move |_| {
            tx0.send(Event::Dummy).unwrap();
        }));

        runner.get_single_sync_job(tx.clone(), move |job| {
            fetch_barrier.0.wait();
            assert_eq!(first_job_id, job.id);
            return_barrier.0.wait();
            Ok(())
        });

        fetch_barrier2.0.wait();
        runner.get_single_sync_job(tx.clone(), move |job| {
            assert_eq!(second_job_id, job.id);
            return_barrier2.0.wait();
            Ok(())
        });
        smol::block_on(runner.wait_for_all_tasks(rx, 2));
    }

    #[test]
    fn jobs_are_deleted_when_successfully_run() {
        crate::initialize();
        let _guard = TestGuard::lock();

        let (tx, rx) = channel::bounded(1);

        let mut runner = runner();
        let tx0 = tx.clone();
        runner.on_finish = Some(Arc::new(move |_| {
            tx0.send(Event::Dummy).unwrap();
        }));
        create_dummy_job(&runner);

        smol::block_on(async move {
            let mut conn = runner.connection().await.unwrap();
            runner.get_single_sync_job(tx.clone(), move |_| Ok(()));
            runner.wait_for_all_tasks(rx, 1).await;
            let remaining_jobs = get_job_count(&mut conn).await;
            assert_eq!(0, remaining_jobs);
        });
    }

    #[test]
    fn panicking_in_sync_jobs_updates_retry_counter() {
        crate::initialize();
        let _guard = TestGuard::lock();
        let mut runner = runner();
        let job_id = create_dummy_job(&runner);
        let (tx, rx) = channel::bounded(3);
        let tx0 = tx.clone();
        runner.on_finish = Some(Arc::new(move |_| {
            tx0.send(Event::Dummy).unwrap();
        }));
        runner.get_single_sync_job(tx.clone(), move |_| panic!());
        smol::block_on(runner.wait_for_all_tasks(rx, 1));

        let mut conn = smol::block_on(runner.connection()).unwrap();
        let tries = smol::block_on(
            sqlx::query_as::<_, (i32,)>("SELECT retries FROM _background_tasks WHERE id = $1")
                .bind(job_id)
                .fetch_one(&mut conn),
        )
        .unwrap()
        .0;
        assert_eq!(1, tries);
    }
}
