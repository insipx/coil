use crate::dummy_jobs::*;
use crate::test_guard::TestGuard;
// use diesel::prelude::*;
// use failure::Fallible;
// use swirl::db::DieselPoolObj;
use anyhow::Result;
use coil::{FailedJobsError::JobsFailed, PerformError};

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
    smol::run(async {
        let mut conn = runner.connection_pool().acquire().await.unwrap();
        check_arg_equal_to_env("a".into()).enqueue(&mut conn).await.unwrap();
        check_arg_equal_to_env("b".into()).enqueue(&mut conn).await.unwrap();
        runner.run_all_sync_tasks().await.unwrap();
    });

    assert_eq!(Err(JobsFailed(1)), smol::block_on(runner.check_for_failed_jobs(rx, 2)));
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
    smol::run(async {
        let mut conn = runner.connection_pool().acquire().await.unwrap();
        assert_foo("foo".into()).enqueue(&mut conn).await.unwrap();
        assert_foo("not foo".into()).enqueue(&mut conn).await.unwrap();
        runner.run_all_sync_tasks().await.unwrap();
    });
    assert_eq!(Err(JobsFailed(1)), smol::block_on(runner.check_for_failed_jobs(rx, 2)));
}


#[test]
fn env_can_have_any_name() {
    #[coil::background_job]
    fn env_with_different_name(environment: &String) -> Result<(), coil::PerformError> {
        assert_eq!(environment, "my environment");
        Ok(())
    }

    let (runner, rx) = TestGuard::runner(String::from("my environment"), 1);
    smol::run(async {
        let mut conn = runner.connection_pool().acquire().await.unwrap();
        env_with_different_name().enqueue(&mut conn).await.unwrap();

        runner.run_all_sync_tasks().await.unwrap();
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
    smol::run(async {
        let mut conn = runner.connection_pool().acquire().await.unwrap();
        uses_trait_import().enqueue(&mut conn).await.unwrap();

        runner.run_all_sync_tasks().await.unwrap();
        runner.check_for_failed_jobs(rx, 1).await.unwrap();
    });
}

/*
#[test]
fn jobs_can_take_a_connection_as_an_argument() -> Fallible<()> {
    use diesel::sql_query;

    #[swirl::background_job]
    fn takes_env_and_conn(_env: &(), conn: &PgConnection) -> Result<(), swirl::PerformError> {
        sql_query("SELECT 1").execute(conn)?;
        Ok(())
    }

    #[swirl::background_job]
    fn takes_only_conn(conn: &PgConnection) -> Result<(), swirl::PerformError> {
        sql_query("SELECT 1").execute(conn)?;
        Ok(())
    }

    #[swirl::background_job]
    fn takes_connection_pool(pool: &dyn DieselPoolObj) -> Result<(), swirl::PerformError> {
        let conn1 = pool.get()?;
        let conn2 = pool.get()?;
        sql_query("SELECT 1").execute(&**conn1)?;
        sql_query("SELECT 1").execute(&**conn2)?;
        Ok(())
    }

    #[swirl::background_job]
    fn takes_fully_qualified_conn(conn: &diesel::PgConnection) -> Result<(), swirl::PerformError> {
        sql_query("SELECT 1").execute(conn)?;
        Ok(())
    }

    #[swirl::background_job]
    fn takes_fully_qualified_pool(
        pool: &dyn swirl::db::DieselPoolObj,
    ) -> Result<(), swirl::PerformError> {
        let conn1 = pool.get()?;
        let conn2 = pool.get()?;
        sql_query("SELECT 1").execute(&**conn1)?;
        sql_query("SELECT 1").execute(&**conn2)?;
        Ok(())
    }

    let runner = TestGuard::dummy_runner();
    {
        let conn = runner.connection_pool().get()?;
        takes_env_and_conn().enqueue(&conn)?;
        takes_only_conn().enqueue(&conn)?;
        takes_connection_pool().enqueue(&conn)?;
        takes_fully_qualified_conn().enqueue(&conn)?;
        takes_fully_qualified_pool().enqueue(&conn)?;
    }

    runner.run_all_pending_jobs()?; runner.check_for_failed_jobs()?;
    Ok(())
}
*/
