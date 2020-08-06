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

//! Database Operations for getting and deleting jobs

use crate::job::{Job, SyncJob};
use sqlx::PgConnection;
use serde::{Serialize, de::DeserializeOwned};
use crate::error::EnqueueError;

// TODO: Should add functionality for retrying failed jobs


pub struct BackgroundJob {
    id: i64,
    job_type: String,
    data: Vec<u8>,
}

pub async fn enqueue_job<T: SyncJob>(conn: &mut PgConnection, job: T) -> Result<(), EnqueueError> {
    let data = rmp_serde::encode::to_vec(&job)?;
    sqlx::query!("INSERT INTO _background_tasks (job_type, data) VALUES ($1, $2)", T::JOB_TYPE, data)
        .execute(conn)
        .await?;
    Ok(())
}

pub async fn find_next_unlocked_job(conn: &mut PgConnection) -> Result<BackgroundJob, EnqueueError> {
    sqlx::query_as!(BackgroundJob, "SELECT id, job_type, data FROM _background_tasks ORDER BY id FOR UPDATE SKIP LOCKED")
        .fetch_one(conn)
        .await
        .map_err(Into::into)
}

pub async fn delete_succesful_job(conn: &mut PgConnection, id: i64) -> Result<(), EnqueueError> {
    sqlx::query!("DELETE FROM _background_tasks WHERE id=$1", id)
        .execute(conn)
        .await?;
    Ok(())
}
