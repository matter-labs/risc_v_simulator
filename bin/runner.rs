use std::io::Read;

use risc_v_simulator::{runner::{run_simple_simulator, DEFAULT_ENTRY_POINT}, sim::SimulatorConfig};

pub fn main() {
    // let args: Vec<String> = std::env::args().collect();
    // dbg!(&args);
    // assert_eq!(args.len(), 2);
    // let path = &args[1];
    println!("ZK RISC-V simulator is starting");

    // let path = "../zk_os/app.bin";
    // let path = "../picorv32/firmware/firmware.bin";
    // let path = "../test_riscv_programs/app.bin";
    // let path = "../zk_ee/zk_os_test_example/app.bin";
    // let path = "../zk_ee/zk_os_test_example/app.bin";
    let path = "../zk_ee/zk_os/app.bin";

    let config = SimulatorConfig::simple(path);


    run_simple_simulator(config);
}
