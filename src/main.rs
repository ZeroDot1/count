use failure::Error;
use hashbrown::HashMap;
use rayon::slice::ParallelSliceMut;
use signal_hook;
use std::{
    cmp::Ord,
    fs::File,
    io::{stdin, stdout, BufRead, BufReader, Write},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use structopt::{
    clap::{_clap_count_exprs, arg_enum},
    StructOpt,
};

arg_enum! {
    #[derive(Debug)]
    enum SortingOrder {
        Key,
        Count,
        None,
    }
}

#[derive(StructOpt, Debug)]
struct Config {
    #[structopt(
        long = "sortby",
        short = "s",
        default_value = "Count",
        raw(
            possible_values = "&SortingOrder::variants()",
            case_insensitive = "true"
        )
    )]
    sort_by: SortingOrder,
    #[structopt(long = "top")]
    top: Option<usize>,
    #[structopt()]
    input: Option<String>,
}

fn create_reader(input: &Option<String>) -> Result<Box<BufRead>, Error> {
    let reader: Box<BufRead> = match input {
        Some(file_name) => Box::new(BufReader::new(File::open(file_name)?)),
        None => Box::new(BufReader::new(stdin())),
    };
    Ok(reader)
}

fn sort_counts<T: Ord + Sync>(counts: &mut Vec<(&String, &T)>, sorting_order: &SortingOrder) {
    match sorting_order {
        SortingOrder::Key => {
            counts.par_sort_unstable_by(|a, b| a.0.cmp(b.0).then(a.1.cmp(b.1).reverse()))
        }
        SortingOrder::Count => {
            counts.par_sort_unstable_by(|a, b| a.1.cmp(b.1).reverse().then(a.0.cmp(b.0)))
        }
        SortingOrder::None => (),
    }
}

fn watch_sig_pipe() -> Result<Arc<AtomicBool>, Error> {
    let sig_pipe = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::SIGPIPE, Arc::clone(&sig_pipe))?;
    Ok(sig_pipe)
}

fn main() -> Result<(), Error> {
    let sig_pipe = watch_sig_pipe()?;

    let config = Config::from_args();

    let reader = create_reader(&config.input)?;

    let mut counter: HashMap<_, u64> = Default::default();

    for line in reader.lines() {
        *counter.entry(line?).or_insert(0) += 1;
    }

    let mut counts: Vec<_> = counter.iter().collect();
    sort_counts(&mut counts, &config.sort_by);

    let n = config.top.unwrap_or_else(|| counts.len());

    let stdout = stdout();
    let mut handle = stdout.lock();
    for (key, count) in counts.iter().take(n) {
        writeln!(handle, "{}\t{}", key, count)?;
        if sig_pipe.load(Ordering::Relaxed) {
            break;
        }
    }

    Ok(())
}
