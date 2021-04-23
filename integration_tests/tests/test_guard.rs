use antidote::{Mutex, MutexGuard};
use coil::{Builder, Runner};
use once_cell::sync::Lazy;
use sqlx::Connection;
use std::ops::{Deref, DerefMut};
use std::time::Duration;
// Since these tests deal with behavior concerning multiple connections
// running concurrently, they have to run outside of a transaction.
// Therefore we can't run more than one at a time.
//
// Rather than forcing the whole suite to be run with `--test-threads 1`,
// we just lock these tests instead.
static TEST_MUTEX: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

pub struct TestGuard<'a, Env: 'static> {
    runner: Runner<Env>,
    _lock: MutexGuard<'a, ()>,
}

impl<'a, Env> TestGuard<'a, Env> {
    pub fn builder(env: Env) -> GuardBuilder<Env> {
        let pg_pool = sqlx::postgres::PgPoolOptions::new()
            .min_connections(10)
            .idle_timeout(std::time::Duration::from_millis(1000))
            .connect_lazy(&crate::DATABASE_URL)
            .unwrap();
        let builder = Runner::builder(env, &pg_pool);
        GuardBuilder { builder }
    }

    pub fn runner(env: Env, tasks: usize) -> (Self, channel::Receiver<coil::Event>) {
        let (tx, rx) = channel::bounded(tasks);

        (
            Self::builder(env)
                .on_finish(move |_| {
                    let _ = tx.send(coil::Event::Dummy);
                })
                .num_threads(4)
                .build(),
            rx,
        )
    }
}

impl<'a> TestGuard<'a, ()> {
    pub fn dummy_runner() -> (Self, channel::Receiver<coil::Event>) {
        let (tx, rx) = channel::unbounded();
        (
            Self::builder(())
                .num_threads(4)
                .on_finish(move |_| {
                    let _ = tx.send(coil::Event::Dummy);
                })
                .build(),
            rx,
        )
    }
}

pub struct GuardBuilder<Env: 'static> {
    builder: Builder<Env>,
}

impl<Env> GuardBuilder<Env> {
    pub fn register_job<T: coil::Job + 'static + Send>(mut self) -> Self {
        self.builder = self.builder.register_job::<T>();
        self
    }

    pub fn num_threads(mut self, threads: usize) -> Self {
        self.builder = self.builder.num_threads(threads);
        self
    }

    /// Provide a hook that runs after a job has finished and all destructors have run
    /// the `on_finish` closure accepts the job ID that finished as an argument
    pub fn on_finish(mut self, on_finish: impl Fn(i64) + Send + Sync + 'static) -> Self {
        self.builder = self.builder.on_finish(on_finish);
        self
    }

    /// Set a timeout in seconds.
    /// This is the maximum amount of time we will wait until classifying a task as a failure and updating the retry counter.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.builder = self.builder.timeout(timeout);
        self
    }

    pub fn build<'a>(self) -> TestGuard<'a, Env> {
        TestGuard {
            _lock: TEST_MUTEX.lock(),
            runner: self.builder.build().unwrap(),
        }
    }
}

impl<'a, Env> Deref for TestGuard<'a, Env> {
    type Target = Runner<Env>;

    fn deref(&self) -> &Self::Target {
        &self.runner
    }
}

impl<'a, Env> DerefMut for TestGuard<'a, Env> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.runner
    }
}

// makes sure all Pg connections are closed and database is empty before running any other tests
impl<'a, Env: 'static> Drop for TestGuard<'a, Env> {
    fn drop(&mut self) {
        smol::block_on(self.runner.connection_pool().close());
        let mut conn = smol::block_on(sqlx::PgConnection::connect(&crate::DATABASE_URL)).unwrap();
        smol::block_on(async {
            sqlx::query("TRUNCATE TABLE _background_tasks")
                .execute(&mut conn)
                .await
                .unwrap()
        });
        smol::block_on(conn.close()).unwrap();
    }
}
