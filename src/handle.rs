use super::config::Xconf;
use super::sys_cfg::{SHELL_ARG, SHELL_CMD};
use std::io;
use std::process::{Command, ExitStatus};

fn exec(sys_cmd: &str) -> io::Result<ExitStatus> {
    Command::new(SHELL_CMD).arg(SHELL_ARG).arg(sys_cmd).status()
}

pub fn run(cmd: &str, x_conf: &Xconf) -> i32 {
    let Some(sys_cmd) = x_conf.get(cmd) else {
        eprintln!("no such command <{}>", cmd);
        return 1;
    };

    match exec(sys_cmd) {
        Ok(status) => status.code().unwrap_or(1),
        Err(err) => {
            eprintln!("failed to execute <{}>: {}", cmd, err);
            1
        }
    }
}
