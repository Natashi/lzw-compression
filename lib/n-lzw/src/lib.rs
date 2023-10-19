use core::fmt;
use std::collections::HashMap;
use std::io::{self};

// WARNING: Don't change these, I didn't make the code work with arbitrary symbol size
const LZW_BITS: usize = 12;
const LZW_SIZE: usize = 1 << LZW_BITS;

const BITSTREAM_PACK_SIMPLE: bool = false;

pub type BitstreamUnit = u16;
pub type Bitstream = Vec<BitstreamUnit>;

type CodeTable = HashMap<Vec<u8>, BitstreamUnit>;
type CodeTableRev = Vec<Vec<u8>>;

pub enum LzwError {
	ErrIO(io::Error),
	ErrCompress(String),
	ErrDecompress(String),
	ErrInvalidLzw,
	ErrOther(String),
}
impl fmt::Display for LzwError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match &self {
			LzwError::ErrIO(e) => write!(f, "IO: {:?}", e.to_string()),
			LzwError::ErrCompress(e) => write!(f, "Compression error: {}", e),
			LzwError::ErrDecompress(e) => write!(f, "Decompression error: {}", e),
			LzwError::ErrInvalidLzw => write!(f, "Invalid LZW file"),
			LzwError::ErrOther(e) => write!(f, "{:?}", e),
		}
	}
}
	
	
pub fn lzw_encode(src: &[u8], callback: impl FnMut(f32)) -> Result<Vec<u8>, LzwError> {
	let bitstream = encode_to_bitstream(src, callback)?;
	let output = bitstream_to_u8vec(&bitstream)?;
	Ok(output)
}
pub fn lzw_decode(src: &[u8], callback: impl FnMut(f32)) -> Result<Vec<u8>, LzwError> {
	let bitstream = u8vec_to_bitstream(src)?;
	let output = decode_from_bitstream(&bitstream, callback)?;
	Ok(output)
}

fn encode_to_bitstream(src: &[u8], mut callback: impl FnMut(f32)) -> Result<Bitstream, LzwError> {
	let mut res: Bitstream = vec![];
	
	// Initialize table with codes 0~255
	fn _create_table() -> CodeTable {
		HashMap::from_iter((0..=255_u8).enumerate()
			.map(|(i, c)| (vec![c], i as BitstreamUnit)))
	}
	let mut dictionary = _create_table();
	
	fn find_code(dict: &CodeTable, current: &[u8]) -> Result<u16, LzwError> {
		let bit = dict.get(current);
		if bit.is_none() {
			return Err(LzwError::ErrCompress(String::from("Code word not found")))
		}
		Ok(*bit.unwrap())
	}
	
	let mut prev: Vec<u8> = vec![];
	let mut i = 0;
	
	for &b in src {
		if dictionary.len() == LZW_SIZE {
			// Reset dictionary
			dictionary = _create_table();
		}
		
		prev.push(b);
		if !dictionary.contains_key(&prev) {
			dictionary.insert(prev.clone(), dictionary.len() as BitstreamUnit);
			
			let code = find_code(&dictionary, &prev[0..(prev.len() - 1)])?;
			res.push(code);
			
			prev = vec![b];
		}
		
		i += 1;
		if i % 10 == 0 {
			callback(res.len() as f32 / src.len() as f32);
			i = 0;
		}
	}
	if !prev.is_empty() {
		let code = find_code(&dictionary, &prev)?;
		res.push(code);
	}
	
	Ok(res)
}
fn decode_from_bitstream(src: &Bitstream, mut callback: impl FnMut(f32)) -> Result<Vec<u8>, LzwError> {
	let mut res: Vec<u8> = vec![];
	
	// Initialize table with codes 0~255
	fn _create_table() -> CodeTableRev {
		Vec::from_iter((0..=255_u8).map(|c| vec![c]))
	}
	let mut dictionary = _create_table();
	
	fn find_code(dict: &CodeTableRev, code: BitstreamUnit) -> Result<&[u8], LzwError> {
		let word = dict.get(code as usize)
			.ok_or(LzwError::ErrInvalidLzw)?;
		Ok(word)
	}
	
	let mut prev: Vec<u8> = vec![];
	let mut i = 0;
	
	for &b in src.iter() {
		if dictionary.len() == LZW_SIZE {
			// Reset dictionary
			dictionary = _create_table();
		}
		
		if b as usize == dictionary.len() {
			prev.push(prev[0]);
			dictionary.push(prev.clone());
		}
		else if !prev.is_empty() {
			let word = find_code(&dictionary, b)?[0];
			prev.push(word);
			dictionary.push(prev.clone());
		}
		
		let word = find_code(&dictionary, b)?.to_owned();
		res.extend(word.clone());
		
		prev = word.to_vec();
		
		i += 1;
		if i % 10 == 0 {
			callback(res.len() as f32 / src.len() as f32);
			i = 0;
		}
	}
	
	Ok(res)
}

fn bitstream_to_u8vec(bitstream: &Bitstream) -> Result<Vec<u8>, LzwError> {
	if bitstream.len() % 2 != 0 {
		return Err(LzwError::ErrCompress(String::from("Uneven bitstream size")));
	}
	
	// Initialize with header
	let mut res: Vec<u8> = vec![b'L', b'Z', b'W', b'S'];
	
	if BITSTREAM_PACK_SIMPLE {
		for i in bitstream.chunks(2) {
			res.push((i[0] & 0xff) as u8);
			res.push(((i[0] >> 8) & 0xff) as u8);
			res.push((i[1] & 0xff) as u8);
			res.push(((i[1] >> 8) & 0xff) as u8);
		}
	}
	else {
		// Bitstream unit = 12 bits
		// 2 units = 24 bits
		// 24 bits -> unpack into 3 bytes
		for i in bitstream.chunks(2) {
			let i0h = ((i[0] >> 8) & 0xff) as u8;
			let i1h = ((i[1] >> 8) & 0xff) as u8;
			res.push((i[0] & 0xff) as u8);
			res.push((i[1] & 0xff) as u8);
			res.push((i1h << 4) | i0h);
		}
	}
	
	Ok(res)
}
fn u8vec_to_bitstream(data: &[u8]) -> Result<Bitstream, LzwError> {
	// Verify header and stream size
	let len = data.len();
	if len < 4 || (len - 4) % 3 != 0 
		|| (data[0] != b'L' || data[1] != b'Z'
			|| data[2] != b'W' || data[3] != b'S') {
		return Err(LzwError::ErrInvalidLzw);
	}
	
	let mut res: Bitstream = vec![];
	
	let skipped = data.iter()
		.skip(4).copied()
		.collect::<Vec<_>>();
	
	if BITSTREAM_PACK_SIMPLE {
		for i in skipped.chunks(4) {
			res.push((i[0] as u16) | ((i[1] as u16) << 8));
			res.push((i[2] as u16) | ((i[3] as u16) << 8));
		}
	}
	else {
		for i in skipped.chunks(3) {
			let i0h = (i[2] & 0xf) as u16;
			let i1h = ((i[2] >> 4) & 0xf) as u16;
			res.push((i[0] as u16) | (i0h << 8));
			res.push((i[1] as u16) | (i1h << 8));
		}
	}
	
	Ok(res)
}
