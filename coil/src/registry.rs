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

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::marker::PhantomData;
use futures::{Future, FutureExt};
use crate::error::PerformError;
use crate::job::Job;
use std::pin::Pin;
use std::sync::Arc;
use sqlx::prelude::*;
use sqlx::Postgres;

type Conn = sqlx::Transaction<'static, Postgres>;

#[derive(Default)]
#[allow(missing_debug_implementations)] // Can't derive debug
/// A registry of background jobs, used to map job types to concrete perform
/// functions at runtime.
pub struct Registry<Env> {
    jobs: HashMap<&'static str, JobVTable>,
    _marker: PhantomData<Env>,
}

impl<Env: 'static> Registry<Env> {
    /// Loads the registry from all invocations of [`register_job!`] for this
    /// environment type
    pub fn load() -> Self {
        let jobs = inventory::iter::<JobVTable>
            .into_iter()
            .filter(|vtable| vtable.env_type == TypeId::of::<Env>())
            .map(|&vtable| (vtable.job_type, vtable))
            .collect();

        Self {
            jobs: jobs,
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

#[derive(Copy, Clone)]
enum SyncOrAsync {
    Sync {
        fun: fn(Vec<u8>, &dyn Any, &mut Conn) -> Result<(), PerformError>
    },
    Async {
        fun: for<'a> fn(Vec<u8>, Arc<(dyn Any + Send + Sync)>, &'a mut Conn) -> Result<Pin<Box<dyn Future<Output = Result<(), PerformError>> + Send + 'a>>, PerformError>
    }
}

impl SyncOrAsync {
    pub fn is_async(&self) -> bool {
        match self {
            SyncOrAsync::Async{..} => true,
            _ => false
        }
    }
}

#[doc(hidden)]
#[derive(Clone, Copy)]
pub struct JobVTable {
    env_type: TypeId,
    job_type: &'static str,
    perform: SyncOrAsync,
}

inventory::collect!(JobVTable);

impl JobVTable {
    pub fn from_job<T: 'static + Job + Send>() -> Self {
        let perform = if T::ASYNC {
            SyncOrAsync::Async {
                fun: perform_async_job::<T>,
            }
        } else {
            SyncOrAsync::Sync {
                fun: perform_sync_job::<T>,
            }
        };
        Self {
            env_type: TypeId::of::<T::Environment>(),
            job_type: T::JOB_TYPE,
            perform,
        }
    }
}

fn perform_sync_job<T: Job>(
    data: Vec<u8>,
    env: &dyn Any,
    conn: &mut Conn,
) -> Result<(), PerformError> {
    let environment = env.downcast_ref().ok_or_else::<PerformError, _>(|| {
        "Incorrect environment type. This should never happen. \
         Please open an issue at https://github.com/paritytech/coil/issues/new"
            .into()
    })?;
    let data = rmp_serde::from_read(data.as_slice())?;
    T::perform(data, environment, conn)
}

fn perform_async_job<'a, T: 'static + Job + Send>(
    data: Vec<u8>,
    env: Arc<(dyn Any + Sync + Send)>,
    conn: &'a mut Conn
) -> Result<Pin<Box<dyn Future<Output = Result<(), PerformError>> + Send + 'a>>, PerformError> {
    let environment = env.downcast().unwrap(); 
    /*ok_or_else::<PerformError, _>(|| {
        "Incorrect environment type. This should never happen. \
         Please open an issue at https://github.com/paritytech/coil/issues/new"
            .into()
    })?;
    */
    let data = rmp_serde::from_read(data.as_slice())?;
    Ok(T::perform_async(data, environment, conn))
}

pub struct PerformJob<Env> {
    vtable: JobVTable,
    _marker: PhantomData<Env>,
}

impl<Env: 'static + Send + Sync> PerformJob<Env> {
    
    pub fn is_async(&self) -> bool {
        matches!(self.vtable.perform, SyncOrAsync::Async { .. })
    }

    /// Perform a job in a synchronous way.
    ///
    /// # Blocks
    /// If the underlying job is async, this method will turn it into a blocking function
    pub fn perform_sync(
        &self,
        data: Vec<u8>,
        env: &Env,
        conn: &mut Conn,
    ) -> Result<(), PerformError> {
        match self.vtable.perform {
            SyncOrAsync::Sync { fun } => {
                fun(data, env, conn)
            },
            SyncOrAsync::Async { fun } => {
                // futures::executor::block_on(fun(data, env, conn)?)
                panic!("Not Async");
            }
        }
    }
    
    /// Perform a job in an asynchronous way
    ///
    /// # Blocks
    /// If the underlying job is synchronous, this method will block
    pub fn perform_async<'a>(
        &self,
        data: Vec<u8>, env: Arc<Env>, conn: &'a mut Conn 
    ) -> Result<Pin<Box<dyn Future<Output = Result<(), PerformError>> + Send + 'a>>, PerformError> {
        match self.vtable.perform {
            SyncOrAsync::Sync { .. } => {
                panic!("Not Sync");
            },
            SyncOrAsync::Async { fun } => {
                fun(data, env, conn)
            }
        }
    }
}
