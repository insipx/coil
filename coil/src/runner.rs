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
use crate::error::Error;

pub struct Builder<Env> {
    environment: Env,
    num_threads: Option<usize>,
    conn: sqlx::PgPool,
    executor: Arc<dyn Spawn>,
    max_tasks: QueueAtOnce,
}

impl<Env> Builder<Env> {
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
    /// maximum number of tasks to run at any one time
    max_tasks: QueueAtOnce
}

/// How many syncronous jobs to queue at any one time
pub enum QueueAtOnce {
    /// Saturate each thread with the number of threads defined for the threadpool (num_cpus)
    Threads,
    /// Saturate threadpool with `size` jobs at any one time
    Size(usize),
    /// Queue all jobs
    All,
}

impl Default for QueueAtOnce {
    fn default() -> QueueAtOnce {
        QueueAtOnce::All
    }
}

impl<Env> Runner<Env> {

    pub fn run_all_pending_tasks(&self) {

    }
    
    fn run_single_job(&self) {

    }
}



