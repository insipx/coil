use crate::dummy_jobs::*;
use crate::test_guard::TestGuard;
use anyhow::Result;
use coil::{FailedJobsError::JobsFailed, PerformError};
use serde::{de::DeserializeOwned, Serialize};
use sqlx::PgPool;

#[test]
fn generated_jobs_serialize_all_arguments_except_first() {
    #[coil::background_job]
    fn check_arg_equal_to_env(env: &String, arg: String) -> Result<(), PerformError> {
        if env == &arg {
            Ok(())
        } else {
            Err("arg wasn't env!".into())
        }
    }

    let (runner, rx) = TestGuard::runner("a".to_string(), 2);
    smol::block_on(async {
        let mut conn = runner.connection_pool().acquire().await.unwrap();
        check_arg_equal_to_env("a".into())
            .enqueue(&mut conn)
            .await
            .unwrap();
        check_arg_equal_to_env("b".into())
            .enqueue(&mut conn)
            .await
            .unwrap();
        runner.run_pending_tasks().unwrap();
    });

    assert_eq!(
        Err(JobsFailed(1)),
        smol::block_on(runner.check_for_failed_jobs(rx, 2))
    );
}

#[test]
fn jobs_with_args_but_no_env() {
    #[coil::background_job]
    fn assert_foo(arg: String) -> Result<(), PerformError> {
        if arg == "foo" {
            Ok(())
        } else {
            Err("arg wasn't foo!".into())
        }
    }

    let (runner, rx) = TestGuard::dummy_runner();
    smol::block_on(async {
        let mut conn = runner.connection_pool().acquire().await.unwrap();
        assert_foo("foo".into()).enqueue(&mut conn).await.unwrap();
        assert_foo("not foo".into())
            .enqueue(&mut conn)
            .await
            .unwrap();
        runner.run_pending_tasks().unwrap();
    });
    assert_eq!(
        Err(JobsFailed(1)),
        smol::block_on(runner.check_for_failed_jobs(rx, 2))
    );
}

#[test]
fn env_can_have_any_name() {
    #[coil::background_job]
    fn env_with_different_name(environment: &String) -> Result<(), coil::PerformError> {
        assert_eq!(environment, "my environment");
        Ok(())
    }

    let (runner, rx) = TestGuard::runner(String::from("my environment"), 1);
    smol::block_on(async {
        let mut conn = runner.connection_pool().acquire().await.unwrap();
        env_with_different_name().enqueue(&mut conn).await.unwrap();

        runner.run_pending_tasks().unwrap();
        runner.check_for_failed_jobs(rx, 1).await.unwrap();
    })
}

#[test]
#[forbid(unused_imports)]
fn test_imports_only_used_in_job_body_are_not_warned_as_unused() {
    use std::io::prelude::*;

    #[coil::background_job]
    fn uses_trait_import() -> Result<(), coil::PerformError> {
        let mut buf = Vec::new();
        buf.write_all(b"foo")?;
        let s = String::from_utf8(buf)?;
        assert_eq!(s, "foo");
        Ok(())
    }

    let (runner, rx) = TestGuard::dummy_runner();
    smol::block_on(async {
        let mut conn = runner.connection_pool().acquire().await.unwrap();
        uses_trait_import().enqueue(&mut conn).await.unwrap();

        runner.run_pending_tasks().unwrap();
        runner.check_for_failed_jobs(rx, 1).await.unwrap();
    });
}

#[test]
fn jobs_can_take_a_connection_as_an_argument() {
    #[coil::background_job]
    fn takes_env_and_conn(_env: &(), pool: &PgPool) -> Result<(), coil::PerformError> {
        smol::block_on(sqlx::query("SELECT 1").execute(pool))?;
        Ok(())
    }

    #[coil::background_job]
    fn takes_just_pool(pool: &PgPool) -> Result<(), coil::PerformError> {
        let mut conn1 = smol::block_on(pool.acquire())?;
        let mut conn2 = smol::block_on(pool.acquire())?;
        smol::block_on(sqlx::query("SELECT 1").execute(&mut conn1))?;
        smol::block_on(sqlx::query("SELECT 1").execute(&mut conn2))?;
        Ok(())
    }

    #[coil::background_job]
    fn takes_fully_qualified_pool(pool: &sqlx::PgPool) -> Result<(), coil::PerformError> {
        let mut conn1 = smol::block_on(pool.acquire())?;
        let mut conn2 = smol::block_on(pool.acquire())?;
        smol::block_on(sqlx::query("SELECT 1").execute(&mut conn1))?;
        smol::block_on(sqlx::query("SELECT 1").execute(&mut conn2))?;
        Ok(())
    }

    let (runner, rx) = TestGuard::dummy_runner();
    smol::block_on(async {
        let mut conn = runner.connection_pool().acquire().await.unwrap();
        takes_env_and_conn().enqueue(&mut conn).await.unwrap();
        takes_just_pool().enqueue(&mut conn).await.unwrap();
        takes_fully_qualified_pool()
            .enqueue(&mut conn)
            .await
            .unwrap();

        runner.run_pending_tasks().unwrap();
        runner.check_for_failed_jobs(rx, 4).await.unwrap();
    });
}

#[test]
fn proc_macro_accepts_arbitrary_where_clauses() {
    #[coil::background_job]
    fn can_specify_where_clause<S>(_eng: &(), arg: S) -> Result<(), coil::PerformError>
    where
        S: Serialize + DeserializeOwned + std::fmt::Display,
    {
        arg.to_string();
        Ok(())
    }

    let (tx, rx) = channel::bounded(1);
    let runner = TestGuard::builder(())
        .register_job::<can_specify_where_clause::Job<String>>()
        .on_finish(move |_| {
            let _ = tx.send(coil::Event::Dummy).unwrap();
        })
        .build();

    smol::block_on(async {
        let mut conn = runner.connection_pool().acquire().await.unwrap();
        can_specify_where_clause("hello".to_string())
            .enqueue(&mut conn)
            .await
            .unwrap();

        runner.run_pending_tasks().unwrap();
        runner.check_for_failed_jobs(rx, 1).await.unwrap();
    });
}
