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
    let path_sym = "../zk_ee/zk_os/app.sym";

    // let mut file = std::fs::File::open(path).expect("must open provided file");
    // let mut buffer = vec![];
    // file.read_to_end(&mut buffer).expect("must read the file");
    //
    let config = SimulatorConfig::simple("../zk_ee/zk_os/app.bin");


    run_simple_simulator(config);
}
