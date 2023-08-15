use std::io::Read;

use risc_v_simulator::runner::run_simple_simulator;

pub fn main() {
    // let args: Vec<String> = std::env::args().collect();
    // dbg!(&args);
    // assert_eq!(args.len(), 2);
    // let path = &args[1];

    let path = "../zk_os/app.bin";

    let mut file = std::fs::File::open(path).expect("must open provided file");
    let mut buffer = vec![];
    file.read_to_end(&mut buffer).expect("must read the file");

    run_simple_simulator(buffer, 1 << 13);
}
