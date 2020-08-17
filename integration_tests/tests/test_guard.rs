use antidote::{Mutex, MutexGuard};
use std::ops::{Deref, DerefMut};
use std::time::Duration;
use coil::{Builder, Runner};
use once_cell::sync::Lazy;
use std::panic::RefUnwindSafe;

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
        let database_url =
            dotenv::var("DATABASE_URL").expect("DATABASE_URL must be set to run tests");
        let pg_pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(16)
            .connect_lazy(&database_url)
            .unwrap();
        let builder = Runner::builder(env, crate::Executor, pg_pool);

        GuardBuilder { builder }
    }

    pub fn runner(env: Env) -> Self {
        Self::builder(env).build()
    }
}

impl<'a> TestGuard<'a, ()> {
    pub fn dummy_runner() -> Self {
        Self::builder(()).build()
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

    pub fn max_tasks(mut self, max_tasks: usize) -> Self {
        self.builder = self.builder.max_tasks(max_tasks);
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
    pub fn timeout(mut self, timeout: std::time::Duration) -> Self {
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

impl<'a, Env: 'static> Drop for TestGuard<'a, Env> {
    fn drop(&mut self) {
        smol::block_on(async move {
            sqlx::query("TRUNCATE TABLE _background_tasks")
                .execute(&mut self.runner.connection().await.unwrap())
                .await
                .unwrap()
        });
    }
}
