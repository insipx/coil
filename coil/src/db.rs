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

use crate::job::Job;
use crate::error::{EnqueueError, Error};
use sqlx::prelude::*;
use sqlx::Postgres;

#[derive(FromRow)]
pub struct BackgroundJob {
    pub id: i64,
    pub job_type: String,
    pub data: Vec<u8>,
    pub is_async: bool,
}

/// Run the migrations for the background tasks.
/// This creates a table _background_tasks which stores the tasks for execution
/// ```sql
/// CREATE TABLE _background_tasks (
///  id BIGSERIAL PRIMARY KEY NOT NULL,
///  job_type TEXT NOT NULL,
///  data BYTEA NOT NULL,
///  is_async BOOLEAN NOT NULL,
///  retries INTEGER NOT NULL DEFAULT 0,
///  last_retry TIMESTAMP NOT NULL DEFAULT '1970-01-01',
///  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
/// );
/// ```
pub async fn migrate(pool: impl Acquire<'_, Database=Postgres>) -> Result<(), Error> {
    sqlx::migrate!("./migrations").run(pool).await.map_err(Into::into)
}

pub async fn enqueue_job<T: Job>(conn: impl Executor<'_, Database=Postgres>, job: T) -> Result<(), EnqueueError> {
    let data = rmp_serde::encode::to_vec(&job)?;
    sqlx::query("INSERT INTO _background_tasks (job_type, data, is_async) VALUES ($1, $2, $3)") 
        .bind(T::JOB_TYPE) 
        .bind(data)
        .bind(T::ASYNC)
        .execute(conn)
        .await?;
    Ok(())
}

/// Get the next unlocked job. 
/// Optionally pass a boolean to specify whether to get the next unlocked synchronous or
/// asynchronous job. 
/// Passing `None` gets the next unlocked job regardless of whether it is async or sync.
pub async fn find_next_unlocked_job(conn: impl Executor<'_, Database=Postgres>, is_async: Option<bool>)  -> Result<Option<BackgroundJob>, EnqueueError> {
    if let Some(a) = is_async {
        match a {
            true => sqlx::query_as::<_, BackgroundJob>("SELECT id, job_type, data, is_async FROM _background_tasks WHERE is_async = true ORDER BY id FOR UPDATE SKIP LOCKED").fetch_optional(conn).await.map_err(Into::into),
            false => sqlx::query_as::<_, BackgroundJob>("SELECT id, job_type, data, is_async FROM _background_tasks WHERE is_async = false ORDER BY id FOR UPDATE SKIP LOCKED").fetch_optional(conn).await.map_err(Into::into),
        }
    } else {
        sqlx::query_as::<_, BackgroundJob>("SELECT id, job_type, data, is_async FROM _background_tasks ORDER BY id FOR UPDATE SKIP LOCKED")
            .fetch_optional(conn)
            .await
            .map_err(Into::into)
    }
}

pub async fn delete_successful_job(conn: impl Executor<'_, Database=Postgres>, id: i64) -> Result<(), EnqueueError> {
    sqlx::query("DELETE FROM _background_tasks WHERE id=$1")
        .bind(id)
        .execute(conn)
        .await?;
    Ok(())
}

pub async fn update_failed_job(conn: impl Executor<'_, Database=Postgres>, id: i64) {
    let _ = sqlx::query("UPDATE _background_tasks SET retries = retries + 1, last_retry = NOW() WHERE id = $1")
        .bind(id)
        .execute(conn)
        .await;
}

