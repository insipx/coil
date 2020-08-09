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
use futures::{StreamExt, future::FutureExt};
use crate::{db, error::*, registry::Registry};

pub struct Builder<Env> {
    environment: Env,
    num_threads: Option<usize>,
    conn: sqlx::PgPool,
    executor: Arc<dyn Spawn>,
    max_tasks: QueueAtOnce,
}

impl<Env: 'static> Builder<Env> {
    pub fn new(env: Env, executor: impl Spawn + 'static, conn: sqlx::PgPool) -> Self {
        let max_tasks = QueueAtOnce::default();
        Self {
            environment: env,
            conn,
            executor: Arc::new(executor),
            max_tasks,
            num_threads: None,
        }
    }

    pub fn num_threads(mut self, threads: usize) -> Self {
        self.num_threads = Some(threads);
        self
    }

    pub fn max_tasks(mut self, max_tasks: QueueAtOnce) -> Self {
        self.max_tasks = max_tasks;
        self
    }

    pub fn build(self) -> Result<Runner<Env>, Error> {
        let pool = if let Some(t) = self.num_threads {
            rayon::ThreadPoolBuilder::new().num_threads(t).thread_name(|i| format!("bg-{}", i)).build()?
        } else {
            rayon::ThreadPoolBuilder::new().thread_name(|i| format!("bg-{}", i)).build()?
        };
        Ok(Runner {
            pool,
            executor: self.executor,
            conn: self.conn,
            environment: Arc::new(self.environment),
            registry: Arc::new(Registry::load()),
            max_tasks: self.max_tasks
        })
    }
}

/// Runner for background tasks
/// Syncronous tasks are ran in a threadpool
/// Asyncronous tasks are spawned on the executor
pub struct Runner<Env> {
    pool: rayon::ThreadPool, 
    executor: Arc<dyn Spawn>,
    conn: PgPool,
    environment: Arc<Env>,
    registry: Arc<Registry<Env>>,
    /// maximum number of tasks to run at any one time
    max_tasks: QueueAtOnce
}

/// How many syncronous jobs to queue at any one time
pub enum QueueAtOnce {
    /// Saturate each thread with the number of threads defined for the threadpool (num_cpus)
    Threads,
    /// Saturate threadpool with `size` jobs at any one time
    Size(usize),
}

impl Default for QueueAtOnce {
    fn default() -> QueueAtOnce {
        QueueAtOnce::Size(500)
    }
}

enum Event {
    Working,
    NoJobAvailable,
    // Errors
}

impl<Env: Send + Sync + 'static> Runner<Env> {

    pub async fn run_all_pending_tasks(&self) -> Result<(), Error> {
        let num_tasks = match self.max_tasks {
            QueueAtOnce::Threads => {
                self.pool.current_num_threads()
            },
            QueueAtOnce::Size(s) => s,
        };
        
        let (tx, mut rx) = flume::bounded(num_tasks);

        let mut pending_messages = 0;
        
        loop {
            let jobs_to_queue = if pending_messages == 0 {
                num_tasks
            } else {
                num_tasks - pending_messages
            };

            for _ in 0..jobs_to_queue {
                self.run_single_job(tx.clone()).await?;
            }
            
            pending_messages += jobs_to_queue;
            let timeout = timer::Delay::new(std::time::Duration::from_secs(5));
            futures::select!{
                msg = rx.next() => {
                    match msg {
                        Some(Event::Working) => pending_messages -=1,
                        Some(Event::NoJobAvailable) => return Ok(()),
                        None => return Err(CommError::NoMessage.into())
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
                tx.send_async(Event::Working).await.unwrap();
                j 
            },
            Ok(None) => {
                tx.send_async(Event::NoJobAvailable).await.unwrap();
                return Ok(());
            },
            Err(e) =>  {
                println!("{:?}", e);
                panic!("ERROR!");
            }
        };

        let perform_fn = registry.get(&job.job_type)
            .ok_or_else(|| PerformError::from(format!("Unknown Job Type {}", job.job_type)))?;
        
        if perform_fn.is_async() {
            self.executor.spawn(async move {
                perform_fn.perform_async(job.data, env, &mut transaction).unwrap().await.unwrap();
                db::delete_succesful_job(&mut transaction, job.id).await.unwrap();
                transaction.commit().await.unwrap();
            })?;
        } else {
            self.pool.spawn_fifo(move || {
                perform_fn.perform_sync(job.data, &env, &mut transaction).unwrap();
                futures::executor::block_on(db::delete_succesful_job(&mut transaction, job.id)).unwrap();
                futures::executor::block_on(transaction.commit()).unwrap();
            });
        }
        Ok(())
    }
}

