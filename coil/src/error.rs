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
}

#[derive(Debug, Error)]
pub enum EnqueueError {
    /// An error occurred while trying to insert the task into Postgres
    #[error("Error inserting task {0}")]
    Sql(#[from] sqlx::Error),
    #[error("Error encoding task for insertion {0}")]
    Encode(#[from] rmp_serde::encode::Error)
}

#[derive(Debug, Error)]
pub enum PerformError {
    #[error("Error decoding task data {0}")]
    Decode(#[from] rmp_serde::decode::Error),
    #[error("Trying to perform a async job in a sync context or vice-versa")]
    WrongJob,
    #[error("{0}")]
    General(String),
}

impl From<&str> for PerformError {
    fn from(err: &str) -> PerformError {
        PerformError::General(err.to_string())
    }
}

