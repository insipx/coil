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

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    /// Error building the Rayon threadpool
    #[error("Building pool failed {0}")]
    Build(#[from] rayon::ThreadPoolBuildError),
    /// Error Enqueing a task for execution later
    #[error(transparent)]
    Enqueue(#[from] EnqueueError),
    /// Error performing a task
    #[error(transparent)]
    Perform(#[from] PerformError),
    /// Error Fetching a task for execution on a threadpool/executor
    #[error(transparent)]
    Fetch(#[from] FetchError),
    /// Error executing SQL
    #[error("error getting connection to db {0}")]
    SQL(#[from] sqlx::Error),
    /// Error occuring as a result of trying to spawn a future onto an executor
    #[error("Couldn't spawn onto executor {0}")]
    Spawn(#[from] futures::task::SpawnError),
    /// Error occured while trying to run the migrations for coil
    #[error("Migrations could not be run {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
}

#[derive(Debug, Error)]
pub enum FetchError {
    #[error("Got no response from worker")]
    NoMessage,
    #[error("Timeout reached while waiting for worker to finish")]
    Timeout,
    #[error("Couldn't load job from storage {0}")]
    FailedLoadingJob(#[from] sqlx::Error),
}

#[derive(Debug, Error)]
pub enum EnqueueError {
    /// An error occurred while trying to insert the task into Postgres
    #[error("Error inserting task {0}")]
    Sql(#[from] sqlx::Error),
    /// Error encoding job arguments
    #[error("Error encoding task for insertion {0}")]
    Encode(#[from] rmp_serde::encode::Error),
}

/// Catch-all error for jobs
pub type PerformError = Box<dyn std::error::Error + Send + Sync>;

#[doc(hidden)]
#[cfg(any(test, feature = "test_components"))]
#[derive(Debug, PartialEq)]
pub enum FailedJobsError {
    /// Jobs that failed to run
    JobsFailed(
        /// Number of failed jobs
        i64,
    ),
}
