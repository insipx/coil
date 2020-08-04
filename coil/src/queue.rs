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

use serde::{Serialize, Deserialize};

/// The task that is inserted and retrieved from the database
#[derive(Serialize, Deserialize)]
pub struct Task {
    /// ID of the job
    id: u32,
    /// Type of the Job
    job_type: String,
    /// MSGPack Encoded Job Data
    data: Vec<u8>
}
