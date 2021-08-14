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

#![allow(clippy::new_without_default)] // https://github.com/rust-lang/rust-clippy/issues/3632

use crate::error::PerformError;
use crate::job::Job;
use sqlx::PgPool;
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::marker::PhantomData;

#[derive(Default)]
#[allow(missing_debug_implementations)] // Can't derive debug
/// A registry of background jobs, used to map job types to concrete perform
/// functions at runtime.
pub struct Registry<Env> {
    jobs: HashMap<&'static str, JobVTable>,
    _marker: PhantomData<Env>,
}

impl<Env: 'static> Registry<Env> {
    pub fn register_job<T: Job + 'static + Send>(&mut self) {
        if TypeId::of::<T::Environment>() == TypeId::of::<Env>() {
            self.jobs.insert(T::JOB_TYPE, JobVTable::from_job::<T>());
        } else {
            log::warn!("could not register job {}", T::JOB_TYPE);
        }
    }

    /// Loads the registry from all invocations of [`register_job!`] for this
    /// environment type
    pub fn load() -> Self {
        let jobs = inventory::iter::<JobVTable>
            .into_iter()
            .filter(|vtable| vtable.env_type == TypeId::of::<Env>())
            .map(|&vtable| (vtable.job_type, vtable))
            .collect();

        Self {
            jobs,
            _marker: PhantomData,
        }
    }

    /// Get the perform function for a given job type
    pub fn get(&self, job_type: &str) -> Option<PerformJob<Env>> {
        self.jobs.get(job_type).map(|&vtable| PerformJob {
            vtable,
            _marker: PhantomData,
        })
    }
}

/// Register a job to be run by coil. This must be called for any
/// implementors of [`coil::Job`]
#[macro_export]
macro_rules! register_job {
    ($job_ty: ty) => {
        $crate::inventory::submit! {
            #![crate = coil]
            coil::JobVTable::from_job::<$job_ty>()
        }
    };
}

#[doc(hidden)]
#[derive(Clone, Copy)]
pub struct JobVTable {
    env_type: TypeId,
    job_type: &'static str,
    perform: fn(serde_json::Value, &dyn Any, &PgPool) -> Result<(), PerformError>,
}

inventory::collect!(JobVTable);

impl JobVTable {
    pub fn from_job<T: 'static + Job + Send>() -> Self {
        Self {
            env_type: TypeId::of::<T::Environment>(),
            job_type: T::JOB_TYPE,
            perform: perform_sync_job::<T>,
        }
    }
}

fn perform_sync_job<T: Job>(
    data: serde_json::Value,
    env: &dyn Any,
    conn: &PgPool,
) -> Result<(), PerformError> {
    let environment = env.downcast_ref().ok_or_else::<PerformError, _>(|| {
        "Incorrect environment type. This should never happen. \
         Please open an issue at https://github.com/paritytech/coil/issues/new"
            .into()
    })?;
    let data = serde_json::from_value(data)?;
    T::perform(data, environment, conn)
}

pub struct PerformJob<Env> {
    vtable: JobVTable,
    _marker: PhantomData<Env>,
}

impl<Env: 'static + Send + Sync> PerformJob<Env> {
    /// Perform a job in a synchronous way.
    pub fn perform_sync(
        &self,
        data: serde_json::Value,
        env: &Env,
        conn: &PgPool,
    ) -> Result<(), PerformError> {
        (self.vtable.perform)(data, env, conn)
    }
}
