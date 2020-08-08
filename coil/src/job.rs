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

use crate::error::{EnqueueError, PerformError};
use serde::{Serialize, de::DeserializeOwned};
use sqlx::{PgPool, Postgres};
use std::sync::Arc;
use sqlx::prelude::*;

// TODO: How do we handle sync tasks?
#[async_trait::async_trait]
pub trait Job: Serialize + DeserializeOwned {
    type Environment: 'static + Send + Sync;
    const JOB_TYPE: &'static str;
    #[doc(hidden)] 
    const ASYNC: bool;    

    /// inserts the job into the Postgres Database
    fn enqueue(self, pool: &PgPool) -> Result<(), EnqueueError> {
        println!("YO BOI");
        todo!();
    }
    
    /// Logic for actually running a synchronous job
    #[doc(hidden)] 
    fn perform<'a, E>(self, env: &Self::Environment, conn: E) -> Result<(), PerformError> 
    where
        E: Executor<'a, Database=Postgres>
    {
        Err(PerformError::WrongJob)
    }
    
    /// Logic for running an asynchronous job
    #[doc(hidden)] 
    async fn perform_async<'a, E>(self, env: Arc<Self::Environment>, conn: E) -> Result<(), PerformError> 
    where
        E: Executor<'a, Database=Postgres>
    {
        Err(PerformError::WrongJob)
    }
}
