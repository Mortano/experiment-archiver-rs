//! This example demonstrates a simple experiment that measures the memory bandwidth of the local machine. The
//! experiment itself is a Rust-implementation of [`mbw`](https://github.com/raas/mbw), a simple benchmark for
//! measuring memory bandwidth.

use std::{
    fmt::Display,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use clap::Parser;
use exar::{
    experiment::ExperimentVersion,
    variable::{DataType, GenericValue, Variable},
};

#[derive(Parser)]
struct Args {
    /// The number of bytes to copy
    #[arg(short, long, default_value_t = 1 << 30)]
    byte_count: usize,
}

#[derive(Debug, Copy, Clone)]
enum MbwMethod {
    Memcpy,
    RustCopy,
    MemcpyInBlocks,
}

impl Display for MbwMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MbwMethod::Memcpy => write!(f, "memcpy"),
            MbwMethod::RustCopy => write!(f, "Rust copy"),
            MbwMethod::MemcpyInBlocks => write!(f, "memcpy in blocks"),
        }
    }
}

const DEFAULT_BLOCK_SIZE: usize = 262144;
const BYTES_IN_MIB: usize = 1024 * 1024;

/// Copy `bytes_to_copy` using the given `method`
fn mbw(bytes_to_copy: usize, method: MbwMethod) -> Duration {
    let src: Vec<u8> = vec![0xaa; bytes_to_copy];
    let mut dst = src.clone();

    let timer = Instant::now();
    match method {
        MbwMethod::Memcpy => dst.copy_from_slice(&src),
        MbwMethod::RustCopy => {
            for idx in 0..bytes_to_copy {
                dst[idx] = src[idx];
            }
        }
        MbwMethod::MemcpyInBlocks => {
            for (src_chunk, dst_chunk) in src
                .chunks_exact(DEFAULT_BLOCK_SIZE)
                .zip(dst.chunks_exact_mut(DEFAULT_BLOCK_SIZE))
            {
                dst_chunk.copy_from_slice(src_chunk);
            }
            let remainder_bytes = bytes_to_copy % DEFAULT_BLOCK_SIZE;
            if remainder_bytes > 0 {
                let num_chunks = bytes_to_copy / DEFAULT_BLOCK_SIZE;
                let last_src_chunk = &src[(num_chunks * DEFAULT_BLOCK_SIZE)..];
                let last_dst_chunk = &mut dst[(num_chunks * DEFAULT_BLOCK_SIZE)..];
                last_dst_chunk.copy_from_slice(last_src_chunk);
            }
        }
    }

    timer.elapsed()
}

fn main() -> Result<()> {
    let args = Args::parse();

    // First we define the input and output variables that our experiment will use:
    let input_variables = [
        Variable::new(
            "Bytes".to_string(),
            "The number of bytes that are copied".to_string(),
            DataType::Unit("B".to_string()),
        ),
        Variable::new(
            "Method".to_string(),
            "The copy method that is used".to_string(),
            DataType::Label,
        ),
    ]
    .into_iter()
    .collect();
    let output_variables = [
        Variable::new(
            "Runtime".to_string(),
            "The runtime of the copy operation".to_string(),
            DataType::Unit("s".to_string()),
        ),
        Variable::new(
            "Throughput".to_string(),
            "The memory throughput of the copy operation".to_string(),
            DataType::Unit("B/s".to_string()),
        ),
    ]
    .into_iter()
    .collect();

    // Then we specify our experiment and obtain the current version of the experiment. Versioning happens automatically
    // based on the name of the experiment and its parameters. Each experiment is uniquely identified by a name, if two
    // experiments have the same name but different parameters (e.g. different input or output variables), these are two
    // versions of the same experiment. The current version is always the one matching the parameters passed to `get_current_version`
    // at the time of the most recent call to this function
    let experiment_version = ExperimentVersion::get_current_version(
        "Memory bandwidth".to_string(),
        "Benchmarks the memory bandwidth by copying memory".to_string(),
        ["Your Name".to_string()].into_iter().collect(),
        input_variables,
        output_variables,
    )
    .context("Failed to get experiment version")?;

    // Before we can execute our experiment, we have to fix the values for the input variables. We have one fixed value for the
    // number of bytes passed in as a command line argument, but three different copy methods, so we will have three sets of
    // fixed input variables:
    for method in [
        MbwMethod::Memcpy,
        MbwMethod::RustCopy,
        MbwMethod::MemcpyInBlocks,
    ] {
        let instance = experiment_version
            .make_instance([
                ("Bytes", GenericValue::Numeric(args.byte_count as f64)),
                ("Method", GenericValue::String(method.to_string())),
            ])
            .context("Failed to create instance of experiment")?;

        // Given an instance, we can execute arbitrary code as often as we want, each time creating a `run` of the experiment
        // instance. A run is a set of measurements for all output variables belonging to a specific experiment instance. Since
        // we want some statistically significant results, we will run our benchmark multiple times and create some runs:
        const NUM_RUNS: usize = 10;
        for run_number in 0..NUM_RUNS {
            instance.run(|context| -> Result<()> {
                let duration = mbw(args.byte_count, method);
                let throughput = args.byte_count as f64 / duration.as_secs_f64();
                // Once we have our data, we record it using the `context` object:
                context.add_measurement("Runtime", GenericValue::Numeric(duration.as_secs_f64()));
                context.add_measurement("Throughput", GenericValue::Numeric(throughput));
                // `exar` only logs data into a database but by default doesn't print anything to stdout, so we do so
                // manually 
                eprintln!("{run_number:4} Method: {method:16} Runtime: {:4.6}s Size: {:.3}MiB Throughput: {:6.3}MiB/s", duration.as_secs_f64(), args.byte_count as f64 / BYTES_IN_MIB as f64, throughput / BYTES_IN_MIB as f64);
                Ok(())
            })?;
        }
    }

    // Experiment is done, data is in the database. You can use `exar-cli` to get the data

    Ok(())
}
