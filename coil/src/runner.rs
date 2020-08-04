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

pub struct RunnerBuilder;

pub struct Runner {
    pool: rayon::ThreadPool, 
    conn: PgPool,
    // maximum number of tasks to run at any one time
    max_tasks: usize,
}

impl Runner {

    pub fn run_all_pending_tasks() {
        // get all pending tasks from the database
        // saturate threadpool
    }
}

