use std::process::{Command, ExitCode};

struct Step {
    name: &'static str,
    program: &'static str,
    args: &'static [&'static str],
}

fn steps() -> &'static [Step] {
    &[
        Step {
            name: "format",
            program: "cargo",
            args: &["fmt", "--check"],
        },
        Step {
            name: "lint",
            program: "cargo",
            args: &["clippy", "--all-targets", "--", "-D", "warnings"],
        },
        Step {
            name: "test",
            program: "cargo",
            args: &["test"],
        },
        Step {
            name: "script fit tests",
            program: "python3",
            args: &["-B", "-m", "unittest", "scripts/test_check_script_fit.py"],
        },
    ]
}

fn main() -> ExitCode {
    for step in steps() {
        eprintln!("==> {}", step.name);
        match Command::new(step.program).args(step.args).status() {
            Ok(status) if status.success() => {}
            Ok(status) => {
                eprintln!("{} failed: {status}", step.name);
                return ExitCode::FAILURE;
            }
            Err(err) => {
                eprintln!("failed to run {}: {err}", step.name);
                return ExitCode::FAILURE;
            }
        }
    }

    ExitCode::SUCCESS
}

#[cfg(test)]
fn step_command_pairs() -> Vec<(&'static str, &'static [&'static str])> {
    steps()
        .iter()
        .map(|step| (step.program, step.args))
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn gate_steps_match_github_workflow() {
        assert_eq!(
            super::step_command_pairs(),
            vec![
                ("cargo", &["fmt", "--check"][..]),
                (
                    "cargo",
                    &["clippy", "--all-targets", "--", "-D", "warnings"][..]
                ),
                ("cargo", &["test"][..]),
                (
                    "python3",
                    &["-B", "-m", "unittest", "scripts/test_check_script_fit.py"][..]
                ),
            ]
        );
    }
}
