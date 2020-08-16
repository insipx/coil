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
mod db;
mod registry;
mod job;
mod runner;

#[doc(hidden)]
pub extern crate serde;
#[doc(hidden)]
pub extern crate inventory;
#[doc(hidden)]
pub extern crate async_trait;
#[doc(hidden)]
pub extern crate sqlx;
#[doc(hidden)]
pub use serde::{Deserialize, Serialize, de::DeserializeOwned};

#[doc(hidden)]
pub use registry::JobVTable;

pub use crate::job::*;
pub use crate::error::*;
pub use crate::db::migrate;
pub use coil_proc_macro::*;
pub use crate::runner::{Runner, Builder as RunnerBuilder};


#[cfg(test)]
use std::sync::Once;
#[cfg(test)]
use sqlx::Connection;
#[cfg(test)]
static INIT: Once = Once::new();

#[cfg(test)]
pub fn initialize() {
    INIT.call_once(|| {
        let url = dotenv::var("DATABASE_URL").unwrap();
        println!("Running migrations for {}", url);
        let mut conn = smol::block_on(sqlx::PgConnection::connect(url.as_str())).unwrap();
        smol::block_on(crate::migrate(&mut conn)).unwrap();
    });
}


