mod characters;
mod flags;
mod virtual_machine;

use std::process::exit;
use std::time::Duration;
use virtual_machine::VirtualMachine;

const HEIGHT: usize = 32;

fn main() -> anyhow::Result<()> {
    let flags = flags::Main::from_env_or_exit();
    let mut machine = VirtualMachine::new(&flags.path)?;
    let instruction_count_ptr = &machine.instruction_count as *const u64 as usize;
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(1));
        let instruction_count = unsafe { *(instruction_count_ptr as *const u64) };
        println!("{:.2}", instruction_count as f64 / 1e6);
        exit(0);
    });
    machine.entry();
    Ok(())
}
