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
    #[error("Building pool failed {0}")]
    Build(#[from] rayon::ThreadPoolBuildError),
    #[error(transparent)]
    Enqueue(#[from] EnqueueError),
    #[error(transparent)]
    Perform(#[from] PerformError),
    #[error(transparent)]
    Fetch(#[from] FetchError),
    #[error("error getting connection to db {0}")]
    SQL(#[from] sqlx::Error),
    #[error("Couldn't spawn onto executor {0}")]
    Spawn(#[from] futures::task::SpawnError),
    #[error("Migrations could not be run {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError)
}

#[derive(Debug, Error)]
pub enum FetchError {
    #[error("Got no response from worker")]
    NoMessage,
    #[error("Timeout reached while waiting for worker to finish")]
    Timeout,
    #[error("Couldn't load job from storage {0}")]
    FailedLoadingJob(#[from] sqlx::Error)
}

#[derive(Debug, Error)]
pub enum EnqueueError {
    /// An error occurred while trying to insert the task into Postgres
    #[error("Error inserting task {0}")]
    Sql(#[from] sqlx::Error),
    #[error("Error encoding task for insertion {0}")]
    Encode(#[from] rmp_serde::encode::Error)
}

pub type PerformError = Box<dyn std::error::Error + Send + Sync>;

#[cfg(any(test, feature = "test_components"))]
#[derive(Debug, PartialEq)]
pub enum FailedJobsError {
    /// Jobs that failed to run
    JobsFailed(
        /// Number of failed jobs
        i64
    ),
}
