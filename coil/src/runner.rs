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
use crate::job::Job;
use futures::{StreamExt, future::FutureExt};
use crate::{db, error::*, registry::Registry};

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
    ///  ```
    ///  RunnerBuilder::new(env, executor, conn)
    ///      .register_job::<resize_image::Job<String>>()
    ///  ```
    ///  Register a job for every generic (if they differ):
    ///
    ///  ```
    ///  RunnerBuilder::new(env, executor, conn)
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

    /// Runs all the pending tasks in a loop
    pub async fn run_all_pending_tasks(&self) -> Result<(), Error> {
        let (tx, mut rx) = flume::bounded(self.max_tasks);

        let mut pending_messages = 0;
        
        loop {
            let jobs_to_queue = if pending_messages == 0 {
                self.max_tasks
            } else {
                self.max_tasks - pending_messages
            };
            
            let mut futures = Vec::with_capacity(jobs_to_queue);
            for _ in 0..jobs_to_queue {
                futures.push(self.run_single_job(tx.clone()))
            }
            futures::future::join_all(futures).await;
            
            pending_messages += jobs_to_queue;
            let timeout = timer::Delay::new(std::time::Duration::from_secs(5));
            futures::select! {
                msg = rx.next() => {
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
    
    async fn run_single_job(&self, tx: flume::Sender<Event>) -> Result<(), Error> {
        let env = Arc::clone(&self.environment);
        let registry = Arc::clone(&self.registry);
        let mut transaction = self.conn.begin().await?;
        let job = match db::find_next_unlocked_job(&mut transaction).await {
            Ok(Some(j)) => { 
                let _ = tx.send_async(Event::Working).await;
                j 
            },
            Ok(None) => {
                let _ = tx.send_async(Event::NoJobAvailable).await;
                return Ok(());
            },
            Err(e) =>  {
                let _ = tx.send_async(Event::ErrorLoadingJob(e.into())).await;
                return Ok(());
            }
        };

        let perform_fn = registry.get(&job.job_type)
            .ok_or_else(|| PerformError::from(format!("Unknown Job Type {}", job.job_type)))?;

        // need to unwind this
        if perform_fn.is_async() {
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
        } else {
            self.pool.spawn_fifo(move || {
                match perform_fn.perform_sync(job.data, &env, &mut transaction) {
                    Ok(_) => futures::executor::block_on(db::delete_succesful_job(&mut transaction, job.id)).unwrap(),
                    Err(_) => futures::executor::block_on(db::update_failed_job(&mut transaction, job.id)).unwrap(),
                }
                futures::executor::block_on(transaction.commit()).unwrap();
            });
        }
        Ok(())
    }
}


