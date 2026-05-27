#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    match gateway_switch_lib::try_run_cli_from_args(&args) {
        Ok(Some(code)) => std::process::exit(code),
        Ok(None) => {}
        Err(error) => {
            eprintln!("{}", error);
            std::process::exit(1);
        }
    }
    gateway_switch_lib::run()
}
