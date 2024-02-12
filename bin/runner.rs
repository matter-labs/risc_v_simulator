use std::io::Read;

use risc_v_simulator::runner::run_simple_simulator;

pub fn main() {
    // let args: Vec<String> = std::env::args().collect();
    // dbg!(&args);
    // assert_eq!(args.len(), 2);
    // let path = &args[1];
    println!("Hello, simulator!");

    // let path = "../zk_os/app.bin";
    // let path = "../picorv32/firmware/firmware.bin";
    // let path = "../test_riscv_programs/app.bin";
    // let path = "../zk_ee/zk_os_test_example/app.bin";
    let path = "../zk_ee/zk_os_test_example/app.bin";

    let mut file = std::fs::File::open(path).expect("must open provided file");
    let mut buffer = vec![];
    file.read_to_end(&mut buffer).expect("must read the file");

    run_simple_simulator(buffer, 30000);
}
