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
use sqlx::PgPool;
use futures::Future;

// TODO: How do we handle sync tasks?
pub trait Job: Serialize + DeserializeOwned {
    type Environment: 'static;
    const JOB_TYPE: &'static str;
    const ASYNC: bool;    

    /// inserts the job into the Postgres Database
    fn enqueue(self, pool: &PgPool) -> Result<(), EnqueueError> {
        println!("YO BOI");
        Ok(())
    }
    
    /// Logic for actually running the job
    fn perform(self, env: &Self::Environment, pool: &sqlx::PgPool) -> Result<(), PerformError>;

    fn perform_async(self, env: &Self::Environment, pool: &sqlx::PgPool) -> Box<dyn Future<Output = Result<(), PerformError>>>;
}
