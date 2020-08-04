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


mod error;
mod queries;

use crate::error::Error;
use serde::{Serialize, de::DeserializeOwned};
use sqlx::PgPool;

// TODO: How do we handle sync tasks?
#[async_trait::async_trait]
trait Job: Serialize + DeserializeOwned {
    type Environment: 'static;
    const JOB_TYPE: &'static str;
    
    /// inserts the job into the Postgres Database
    async fn enqueue(self, pool: &PgPool) -> Result<(), Error>;
    /// Logic for actually running the job
    async fn execute(&self);
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
