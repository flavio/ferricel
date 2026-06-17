use cel::common::ast::CallExpr;
use ferricel_types::functions::RuntimeFunction;
use walrus::InstrSeqBuilder;

use crate::compiler::{
    context::{CompilerContext, CompilerEnv},
    expr::compile_expr,
};

/// Compile a temporal function call.
pub fn compile_temporal_function(
    func_name: &str,
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    match func_name {
        "timestamp" => compile_timestamp(call_expr, body, env, ctx, module),
        "duration" => compile_duration(call_expr, body, env, ctx, module),
        "getFullYear" => compile_timestamp_accessor(
            call_expr,
            func_name,
            RuntimeFunction::TimestampGetFullYear,
            RuntimeFunction::TimestampGetFullYearTz,
            body,
            env,
            ctx,
            module,
        ),
        "getMonth" => compile_timestamp_accessor(
            call_expr,
            func_name,
            RuntimeFunction::TimestampGetMonth,
            RuntimeFunction::TimestampGetMonthTz,
            body,
            env,
            ctx,
            module,
        ),
        "getDate" => compile_timestamp_accessor(
            call_expr,
            func_name,
            RuntimeFunction::TimestampGetDate,
            RuntimeFunction::TimestampGetDateTz,
            body,
            env,
            ctx,
            module,
        ),
        "getDayOfMonth" => compile_timestamp_accessor(
            call_expr,
            func_name,
            RuntimeFunction::TimestampGetDayOfMonth,
            RuntimeFunction::TimestampGetDayOfMonthTz,
            body,
            env,
            ctx,
            module,
        ),
        "getDayOfWeek" => compile_timestamp_accessor(
            call_expr,
            func_name,
            RuntimeFunction::TimestampGetDayOfWeek,
            RuntimeFunction::TimestampGetDayOfWeekTz,
            body,
            env,
            ctx,
            module,
        ),
        "getDayOfYear" => compile_timestamp_accessor(
            call_expr,
            func_name,
            RuntimeFunction::TimestampGetDayOfYear,
            RuntimeFunction::TimestampGetDayOfYearTz,
            body,
            env,
            ctx,
            module,
        ),
        "getHours" => compile_timestamp_accessor(
            call_expr,
            func_name,
            RuntimeFunction::TimestampGetHours,
            RuntimeFunction::TimestampGetHoursTz,
            body,
            env,
            ctx,
            module,
        ),
        "getMinutes" => compile_timestamp_accessor(
            call_expr,
            func_name,
            RuntimeFunction::TimestampGetMinutes,
            RuntimeFunction::TimestampGetMinutesTz,
            body,
            env,
            ctx,
            module,
        ),
        "getSeconds" => compile_timestamp_accessor(
            call_expr,
            func_name,
            RuntimeFunction::TimestampGetSeconds,
            RuntimeFunction::TimestampGetSecondsTz,
            body,
            env,
            ctx,
            module,
        ),
        "getMilliseconds" => compile_timestamp_accessor(
            call_expr,
            func_name,
            RuntimeFunction::TimestampGetMilliseconds,
            RuntimeFunction::TimestampGetMillisecondsTz,
            body,
            env,
            ctx,
            module,
        ),
        _ => anyhow::bail!("Unknown temporal function: {}", func_name),
    }
}

/// Compile `timestamp(string)` - parses RFC3339 timestamp string.
fn compile_timestamp(
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    if call_expr.args.len() != 1 {
        anyhow::bail!("timestamp() expects 1 argument (RFC3339 string)");
    }
    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
    body.call(env.get(RuntimeFunction::Timestamp));
    Ok(())
}

/// Compile `duration(string)` - parses CEL duration format string.
fn compile_duration(
    call_expr: &CallExpr,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    if call_expr.args.len() != 1 {
        anyhow::bail!("duration() expects 1 argument (duration string)");
    }
    compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
    body.call(env.get(RuntimeFunction::Duration));
    Ok(())
}

/// Compile a timestamp accessor method with optional timezone parameter.
///
/// Handles the shared pattern for all `get*` methods:
/// - `timestamp.getXxx()` -> call no_tz_fn
/// - `timestamp.getXxx("UTC")` -> compile tz arg, call tz_fn
/// - Must be called as a method on a timestamp (target is required)
#[allow(clippy::too_many_arguments)]
fn compile_timestamp_accessor(
    call_expr: &CallExpr,
    func_name: &str,
    no_tz_fn: RuntimeFunction,
    tz_fn: RuntimeFunction,
    body: &mut InstrSeqBuilder,
    env: &CompilerEnv,
    ctx: &CompilerContext,
    module: &mut walrus::Module,
) -> Result<(), anyhow::Error> {
    if let Some(target) = &call_expr.target {
        compile_expr(&target.expr, body, env, ctx, module)?;
        if call_expr.args.is_empty() {
            body.call(env.get(no_tz_fn));
        } else if call_expr.args.len() == 1 {
            compile_expr(&call_expr.args[0].expr, body, env, ctx, module)?;
            body.call(env.get(tz_fn));
        } else {
            anyhow::bail!(
                "{}() expects 0 or 1 argument (optional timezone)",
                func_name
            );
        }
    } else {
        anyhow::bail!("{}() must be called as a method on a timestamp", func_name);
    }
    Ok(())
}
