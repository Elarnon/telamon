extern crate env_logger;
extern crate getopts;
extern crate telamon;
extern crate telamon_kernels;

use telamon::device;
use telamon::device::x86;
use telamon::explorer::config::Config;
use telamon_kernels::{linalg, Kernel};

#[cfg(feature = "cuda")]
mod cuda_wrapper {
    use telamon::device::cuda;

    pub fn with_cuda_context<F: for<'a> FnOnce(cuda::Context<'a>)>(f: F) {
        let executor = cuda::Executor::init();
        f(cuda::Context::new(&executor));
    }
}

#[cfg(not(feature = "cuda"))]
mod cuda_wrapper {
    use telamon::device::x86;

    pub fn with_cuda_context<F: FnOnce(x86::Context)>(_f: F) {
        panic!("cuda support is not available. Try --device x86.");
    }
}

fn with_x86_context<F: FnOnce(x86::Context)>(f: F) {
    f(x86::Context::new())
}

fn run<C: device::ArgMap + device::Context>(config: &Config, mut context: C) {
    let params = linalg::MatMulP::new(1024, 1024, 1024);
    linalg::MatMul::<f32>::benchmark(&config, params, 0, &mut context);
}

enum Device {
    X86,
    Cuda,
}

fn main() {
    env_logger::init();

    let matches = {
        let mut opts = getopts::Options::new();
        opts.optopt("d", "device", "device to use (x86 or cuda)", "[x86 | cuda]");
        let args: Vec<_> = std::env::args().collect();
        opts.parse(&args[1..]).unwrap_or_else(|err| {
            println!("{}", err);
            std::process::exit(-1);
        })
    };
    let device = if let Some(device_str) = matches.opt_str("d") {
        match device_str.as_ref() {
            "x86" => Device::X86,
            "cuda" => Device::Cuda,
            device_str => {
                println!(
                    "Invalid device {}. Valid devices are: x86 and cuda.",
                    device_str
                );
                std::process::exit(-1);
            }
        }
    } else {
        Device::Cuda
    };

    let config = Config::read_from_file();
    match device {
        Device::X86 => with_x86_context(|context| run(&config, context)),
        Device::Cuda => cuda_wrapper::with_cuda_context(|context| run(&config, context)),
    }
}
