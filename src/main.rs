use std::{env, process::exit, fs::File};
use std::io::{Read, Write};
use std::time::{Instant, Duration};

use indicatif::ProgressBar;

use n_lzw::*;

struct WorkResult {
	size_before: usize,
	size_after: usize,
	elapsed: Duration,
}

fn main() {
	macro_rules! wrap_io {
		( $wrp:expr ) => {
			$wrp.map_err(|e| LzwError::ErrIO(e))
		};
	}
	
	let argv: Vec<String> = env::args().collect();
	if argv.len() < 4 {
		print_help_and_exit();
	}
	
	let mode = &argv[1].as_bytes()[0];
	match *mode as char {
		'c' | 'd' => {
			let compress = *mode == b'c';
			
			let path_in = &argv[2];
			let path_out = &argv[3];
			
			let _work = || -> Result<WorkResult, LzwError> {
				let mut file_in = wrap_io!(File::open(path_in))?;
				let mut file_out = wrap_io!(File::create(path_out))?;
				
				let mut file_data: Vec<u8> = vec![];
				wrap_io!(file_in.read_to_end(&mut file_data))?;
				
				let bar = ProgressBar::new(file_data.len() as u64);
				let bar_inc_callback = |f| {
					bar.set_position((f * file_data.len() as f32) as u64);
				};
				
				let time = Instant::now();
				
				let lzw_data = (if compress { 
						lzw_encode(&file_data, bar_inc_callback) 
					}
					else {
						lzw_decode(&file_data, bar_inc_callback) 
					})?;
				wrap_io!(file_out.write_all(&lzw_data))?;
				
				bar.finish();
				
				Ok(WorkResult {
					size_before: file_data.len(),
					size_after: lzw_data.len(),
					elapsed: time.elapsed(),
				})
			};
			match _work() {
				Err(e) => print_and_exit(&format!("Failure-> {}", e)),
				Ok(r) => {
					println!("Success");
					
					println!("Elapsed time: {:.2?}", r.elapsed);
					if compress {
						println!("    Original size:   {}", r.size_before);
						println!("    Compressed size: {}", r.size_after);
						println!("    Compression ratio: {:.2}%", 
							(r.size_after as f64) / (r.size_before as f64) * 100.0);
					}
					else {
						println!("    Compressed size:   {}", r.size_before);
						println!("    Uncompressed size: {}", r.size_after);
					}
				},
			}
		},
		_ => print_help_and_exit(),
	};
}

fn print_help_and_exit() {
	print_and_exit(r#"
Format: MODE ARGS...
    MODE can be:
        c [INPUT] [OUTPUT]
            Compresses a file
        d [INPUT] [OUTPUT]
            Decompresses a file"#
	);
}
fn print_and_exit(s: &str) {
	println!("{}", s);
	exit(-1);
}