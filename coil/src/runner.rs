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

use sqlx::PgPool;
use futures::task::{Spawn, SpawnExt};
use std::sync::Arc;
use std::panic::UnwindSafe;
use crate::job::Job;
use futures::{Future, StreamExt, future::FutureExt, executor::block_on};
use crate::{db, error::*, registry::Registry};
use std::pin::Pin;
use sqlx::{Postgres, Executor};
use channel::{Sender, Receiver};

/// Builder pattern struct for the Runner
pub struct Builder<Env> {
    environment: Env,
    num_threads: Option<usize>,
    conn: sqlx::PgPool,
    executor: Arc<dyn Spawn>,
    max_tasks: Option<usize>,
    registry: Registry<Env>,
}

impl<Env: 'static> Builder<Env> {
    pub fn new(env: Env, executor: impl Spawn + 'static, conn: sqlx::PgPool) -> Self {
        Self {
            environment: env,
            conn,
            executor: Arc::new(executor),
            max_tasks: None,
            num_threads: None,
            registry: Registry::load(),
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
    ///  RunnerBuilder::new(env, executor, conn)
    ///      .register_job::<resize_image::Job<String>>()
    ///  ```
    ///  Register a job for every generic (if they differ):
    ///
    ///  ```ignore
    ///  RunnerBuilder::new((), executor, conn)
    ///     .register_job::<resize_image::Job<String>>()
    ///     .register_job::<resize_image::Job<u32>>()
    ///     .register_job::<resize_image::Job<MyStruct>()
    ///  ```
    ///
    pub fn register_job<T: Job + 'static + Send>(mut self) -> Self {
        self.registry.register_job::<T>();
        self
    }

    pub fn num_threads(mut self, threads: usize) -> Self {
        self.num_threads = Some(threads);
        self
    }

    pub fn max_tasks(mut self, max_tasks: usize) -> Self {
        self.max_tasks = Some(max_tasks);
        self
    }

    pub fn build(self) -> Result<Runner<Env>, Error> {
        let pool = if let Some(t) = self.num_threads {
            rayon::ThreadPoolBuilder::new().num_threads(t).thread_name(|i| format!("bg-{}", i)).build()?
        } else {
            rayon::ThreadPoolBuilder::new().thread_name(|i| format!("bg-{}", i)).build()?
        };
        let max_tasks = if let Some(max) = self.max_tasks {
            max
        } else {
            pool.current_num_threads()
        };
        
        Ok(Runner {
            pool,
            executor: self.executor,
            conn: self.conn,
            environment: Arc::new(self.environment),
            registry: Arc::new(self.registry),
            // registry: Arc::new(Registry::load()),
            max_tasks 
        })
    }
}

/// Runner for background tasks.
/// Synchronous tasks are run in a threadpool.
/// Asynchronous tasks are spawned on the executor.
pub struct Runner<Env> {
    pool: rayon::ThreadPool, 
    executor: Arc<dyn Spawn>,
    conn: PgPool,
    environment: Arc<Env>,
    registry: Arc<Registry<Env>>,
    /// maximum number of tasks to run at any one time
    max_tasks: usize 
}

enum Event {
    Working,
    NoJobAvailable,
    ErrorLoadingJob(Error),
}

impl<Env: Send + Sync + 'static> Runner<Env> {
    
    /// Build the builder for `Runner`
    pub fn build(env: Env, executor: impl Spawn + 'static, conn: sqlx::PgPool) -> Builder<Env> {
        Builder::new(env, executor, conn)
    }

    pub async fn connection(&self) -> Result<sqlx::pool::PoolConnection<sqlx::Postgres>, Error> {
        let conn = self.conn.acquire().await?;
        Ok(conn)
    }

    /// Runs all the pending tasks in a loop
    pub async fn run_all_pending_tasks(&self) -> Result<(), Error> {
        let (tx, mut rx) = channel::bounded(self.max_tasks);

        let mut pending_messages = 0;
        
        loop {
            let jobs_to_queue = if pending_messages == 0 {
                self.max_tasks
            } else {
                self.max_tasks - pending_messages
            };
            
            let mut futures = Vec::with_capacity(jobs_to_queue);
            for _ in 0..jobs_to_queue {
                // futures.push(self.run_single_job(tx.clone()))
                futures.push(async move {
                    println!("Hello");
                });
            }
            futures::future::join_all(futures).await;
            
            pending_messages += jobs_to_queue;
            let timeout = timer::Delay::new(std::time::Duration::from_secs(5));
            futures::select! {
                msg = rx.next().fuse() => {
                    match msg {
                        Some(Event::Working) => pending_messages -=1,
                        Some(Event::NoJobAvailable) => return Ok(()),
                        None => return Err(CommError::NoMessage.into()),
                        _ => todo!()
                    }
                },
                _ = timeout.fuse() => return Err(CommError::NoMessage.into())
            };
        }
    }

    fn run_single_sync_job(&self, tx: Sender<Event>) -> Result<(), Error> {
        todo!() 
        /*
        self.pool.spawn_fifo(move || {
            match perform_fn.perform_sync(job.data, &env, &mut transaction) {
                Ok(_) => futures::executor::block_on(db::delete_succesful_job(&mut transaction, job.id)).unwrap(),
                Err(_) => futures::executor::block_on(db::update_failed_job(&mut transaction, job.id)).unwrap(),
            }
            futures::executor::block_on(transaction.commit()).unwrap();
        });
        */
    }
/*
    async fn run_single_async_job(&self, tx: flume::Sender<Event>) -> Result<(), Error> 
    {
        // let mut transaction = self.conn.begin().await?;
        // let job = Self::get_single_job(tx, &mut transaction, Some(true)).await?;
        
        let job = if job.is_none() {
            return Ok(());
        } else {
            job.expect("Checked for none; qed")
        };
        
        let env = Arc::clone(&self.environment);
        let registry = Arc::clone(&self.registry);

        let perform_fn = registry.get(&job.job_type)
            .ok_or_else(|| PerformError::UnknownJob(job.job_type.to_string()))?;

        // need to unwind this
        self.executor.spawn(async move {
            println!("Spawned");
            let res = perform_fn.perform_async(job.data, env, &mut transaction).await;
            match  res {
                Ok(_) => db::delete_succesful_job(&mut transaction, job.id).await.unwrap(),
                Err(e) => {
                    println!("{:?}", e);
                    db::update_failed_job(&mut transaction, job.id).await.unwrap()
                },
            }
            transaction.commit().await.unwrap();
        })?;
        Ok(())
    }
*/

    async fn get_single_async_job<F>(&self, tx: Sender<Event>, fun: F) -> Result<(), PerformError> 
    where
        F: FnOnce(db::BackgroundJob) -> Pin<Box<dyn Future<Output = Result<(), PerformError>> + Send>> + Send + UnwindSafe + 'static
    {
        let conn = self.conn.clone();
        self.executor.spawn_with_handle(async move {
            let mut transaction = match conn.begin().await {
                    Ok(t) => t,
                    Err(e) => {
                        let fut = tx.send(Event::ErrorLoadingJob(e.into()));
                        fut.await;
                        return Ok(());
                    }
            };
            let job = match db::find_next_unlocked_job(&mut transaction, Some(true)).await {
                Ok(Some(j)) => { 
                    let _ = tx.send(Event::Working).await;
                    j 
                },
                Ok(None) => {
                    let _ = tx.send(Event::NoJobAvailable).await;
                    return Ok(());
                },
                Err(e) =>  {
                    let _ = tx.send(Event::ErrorLoadingJob(e.into())).await;
                    return Ok(());
                }
            };
            fun(job).await
        });
        Ok(())
    }

    fn get_single_sync_job<F>(&self, tx: Sender<Event>, fun: F) 
    where
        F: FnOnce(db::BackgroundJob) -> Result<(), PerformError> + Send + UnwindSafe + 'static
    {
        let conn = self.conn.clone();
        self.pool.spawn_fifo(move || {
            let res = move || -> Result<(), PerformError> {
                let mut transaction = match block_on(conn.begin()) {
                    Ok(t) => t,
                    Err(e) => {
                        let _ = block_on(tx.send(Event::ErrorLoadingJob(e.into())));
                        return Ok(());
                    }
                };
                let job = match block_on(db::find_next_unlocked_job(&mut transaction, Some(false))) {
                    Ok(Some(j)) => { 
                        let _ = block_on(tx.send(Event::Working));
                        j 
                    },
                    Ok(None) => {
                        let _ = block_on(tx.send(Event::NoJobAvailable));
                        return Ok(());
                    },
                    Err(e) =>  {
                        let _ = block_on(tx.send(Event::ErrorLoadingJob(e.into())));
                        return Ok(());
                    }
                };
                fun(job)
            };
            res().unwrap()
        });
    }

    /*
    async fn get_single_job<F>(&self, tx: Sender<Event>, is_async: Option<bool>, fun: F) -> Result<(), PerformError> 
    where
        F: FnOnce(db::BackgroundJob) -> Result<(), PerformError>
    {
        match is_async {
            Some(true) => {
                self.get_single_async_job(tx, |job| async move { fun(job) }.boxed() )
            },
            Some(false) => {
                            },
            None => {
                todo!();
            }
        }
    }
   */ 
    /*
    fn spawn_async<F>(&self, tx: flume::Sender<Event>, conn: impl Executor<'_, Database=Postgres>, fun: F) -> Result<(), Error> 
    where 
        F: FnOnce() -> Result<(), PerformError> + Send + UnwindSafe + 'static,
    {
        self.executor.spawn(async move {
            fun()
        })?;
    }
    */
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier, Mutex, MutexGuard};
    use once_cell::sync::Lazy;
    use sqlx::prelude::*;
    use std::panic::{catch_unwind, AssertUnwindSafe, PanicInfo, RefUnwindSafe, UnwindSafe};


    static TEST_MUTEX: Lazy<Mutex<()>> = Lazy::new(|| {
        Mutex::new(())
    });

    struct TestGuard<'a>(MutexGuard<'a, ()>);
    impl<'a> TestGuard<'a> {
        fn lock() -> Self {
            TestGuard(TEST_MUTEX.lock().unwrap())

        }
    }

    impl<'a> Drop for TestGuard<'a> {
        fn drop(&mut self) {
            smol::block_on(async move {
                sqlx::query!("TRUNCATE TABLE _background_tasks")
                    .execute(&mut runner().connection().await.unwrap())
                    .await
                    .unwrap()
            });
        }
    }

    struct Executor;
    impl futures::task::Spawn for Executor {
        fn spawn_obj(&self, future: futures::task::FutureObj<'static, ()>) -> Result<(), futures::task::SpawnError> {
            smol::Task::spawn(future).detach();
            Ok(())
        }
    }

    fn runner() -> Runner<()> {
        let database_url = dotenv::var("DATABASE_URL").expect("DATABASE_URL must be set to run tests");
        let pool = smol::block_on(sqlx::PgPool::connect(database_url.as_str())).unwrap();
        crate::Runner::build((), Executor, pool)
            .num_threads(2)
            .max_tasks(2)
            .build()
            .unwrap()
    }

    fn create_dummy_job(runner: &Runner<()>) -> i64 {
        let data = rmp_serde::to_vec(vec![0].as_slice()).unwrap();
        smol::block_on(async move {
            let mut conn = runner.connection().await.unwrap();
            let rec = sqlx::query!("INSERT INTO _background_tasks (job_type, data) VALUES ($1, $2) RETURNING (id, job_type, data)", "Foo", data)
                .fetch_one(&mut conn)
                .await
                .unwrap();
            sqlx::query_as::<_, (i64,)>("SELECT currval(pg_get_serial_sequence('_background_tasks', 'id'))")
                .fetch_one(&mut conn)
                .await
                .unwrap()
                .0
        })
    }

    #[test]
    fn jobs_are_locked_when_fetched() {
        let _guard = TestGuard::lock();
        let runner = runner();
        let first_job_id = create_dummy_job(&runner);
        let second_job_id = create_dummy_job(&runner);
        let fetch_barrier = Arc::new(AssertUnwindSafe(Barrier::new(2)));
        let fetch_barrier2 = fetch_barrier.clone();
        let return_barrier = Arc::new(AssertUnwindSafe(Barrier::new(2)));
        let return_barrier2 = return_barrier.clone();
    
        let pool = runner.conn.clone();

        let (tx, _) = channel::bounded(10);
        let tx0 = tx.clone();
        let pool0 = pool.clone();
        let handle0 = smol::block_on(async move {
            let mut transaction = pool0.begin().await.unwrap();
            std::thread::spawn(move ||{
                let job = smol::block_on(Runner::<()>::get_single_job(tx0, &mut transaction)).unwrap().unwrap();
                fetch_barrier.0.wait();
                assert_eq!(first_job_id, job.id);
                return_barrier.0.wait();
            })
        });
        
        fetch_barrier2.0.wait();
        let tx1 = tx.clone();
        let handle1 = smol::block_on(async move {
            let mut transaction = pool.begin().await.unwrap();
            std::thread::spawn(move || {
                let job = smol::block_on(Runner::<()>::get_single_job(tx1, &mut transaction)).unwrap().unwrap();
                assert_eq!(second_job_id, job.id);
                return_barrier2.0.wait();
            })
        });

        handle0.join();
        handle1.join();
    }
}
