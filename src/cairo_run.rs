use std::collections::HashMap;

use cairo_rs::{
	bigint,
	hint_processor::hint_processor_definition::HintProcessor,
	types::{program::Program, relocatable::Relocatable},
	vm::{
		errors::{cairo_run_errors::CairoRunError, vm_errors::VirtualMachineError},
		hook::Hooks,
		runners::cairo_runner::CairoRunner,
		vm_core::VirtualMachine,
	},
};
use num_bigint::BigInt;
use uuid::Uuid;

use crate::{
	hints::{EXECUTION_UUID_VAR_NAME, EXPECT_REVERT_FLAG, MOCK_CALL_FELT_KEY, MOCK_CALL_KEY},
	hooks::HOOKS_VAR_NAME,
};

/// Execute a cairo program
///
/// A `CairoRunner` and a `VirtualMachine` will be created to execute the given `Program`.
/// Hint and `Hooks` (if any) will be applied by the `VirtualMachine`
///
/// When no error is encountered, returns the `CairoRunner` and `VirtualMachine`.
/// Otherwise, returns a `CairoRunError`
///
/// `cairo_run` is the last step after cairo files have been listed and compiled.
/// Each *test* functions will be executed by `cairo_run` with hooks and hints applied.
pub fn cairo_run(
	program: Program,
	hint_processor: &dyn HintProcessor,
	execution_uuid: Uuid,
	opt_hooks: Option<Hooks>,
) -> Result<(CairoRunner, VirtualMachine), CairoRunError> {
	let mut cairo_runner = CairoRunner::new(&program)?;
	let mut vm = VirtualMachine::new(program.prime, false);
	let end = cairo_runner.initialize(&mut vm)?;

	cairo_runner
		.exec_scopes
		.insert_value(EXECUTION_UUID_VAR_NAME, bigint!(execution_uuid.as_u128()));
	if let Some(hooks) = opt_hooks {
		cairo_runner.exec_scopes.insert_value(HOOKS_VAR_NAME, hooks);
	}

	// Init exec context for mock_call
	let mock_felt_hashmap: HashMap<usize, BigInt> = HashMap::new();
	cairo_runner.exec_scopes.insert_value(MOCK_CALL_FELT_KEY, mock_felt_hashmap);

	let mock_call_hashmap: HashMap<usize, (usize, Relocatable)> = HashMap::new();
	cairo_runner.exec_scopes.insert_value(MOCK_CALL_KEY, mock_call_hashmap);

	let execution_result = cairo_runner.run_until_pc(end, &mut vm, hint_processor);
	let should_revert = cairo_runner.exec_scopes.get_any_boxed_ref(EXPECT_REVERT_FLAG).is_ok();

	match execution_result {
		Ok(_) if should_revert => Err(VirtualMachineError::CustomHint(
			EXPECT_REVERT_FLAG.to_string(),
		)),
		Err(_) if should_revert => Ok(()),
		_ => execution_result,
	}
	.map_err(CairoRunError::VirtualMachine)?;

	vm.verify_auto_deductions().map_err(CairoRunError::VirtualMachine)?;

	cairo_runner.relocate(&mut vm).map_err(CairoRunError::Trace)?;

	Ok((cairo_runner, vm))
}
