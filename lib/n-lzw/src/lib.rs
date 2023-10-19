use core::fmt;
use std::collections::HashMap;
use std::io::{self};

// WARNING: Don't change these, I didn't make the code work with arbitrary symbol size
const LZW_BITS: usize = 12;
const LZW_SIZE: usize = 1 << LZW_BITS;

const BITSTREAM_PACK_SIMPLE: bool = false;

pub type SymbolUnit = u16;
pub struct LzwStream {
	data: Vec<SymbolUnit>,
	size: usize,	// Number of symbols
}

type CodeTable = HashMap<Vec<u8>, SymbolUnit>;
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
	let bitstream = encode_to_lzwstream(src, callback)?;
	let output = lzwstream_to_bitstream(&bitstream)?;
	Ok(output)
}
pub fn lzw_decode(src: &[u8], callback: impl FnMut(f32)) -> Result<Vec<u8>, LzwError> {
	let bitstream = bitstream_to_lzwstream(src)?;
	let output = decode_from_lzwstream(&bitstream, callback)?;
	Ok(output)
}

fn encode_to_lzwstream(src: &[u8], mut callback: impl FnMut(f32)) -> Result<LzwStream, LzwError> {
	let mut res: LzwStream = LzwStream {
		data: vec![],
		size: 0,
	};
	
	// Initialize table with codes 0~255
	fn _create_table() -> CodeTable {
		HashMap::from_iter((0..=255_u8).enumerate()
			.map(|(i, c)| (vec![c], i as SymbolUnit)))
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
			dictionary.insert(prev.clone(), dictionary.len() as SymbolUnit);
			
			let code = find_code(&dictionary, &prev[0..(prev.len() - 1)])?;
			res.data.push(code);
			
			prev = vec![b];
		}
		
		i += 1;
		if i % 10 == 0 {
			callback(res.data.len() as f32 / src.len() as f32);
			i = 0;
		}
	}
	if !prev.is_empty() {
		let code = find_code(&dictionary, &prev)?;
		res.data.push(code);
	}
	
	// Data size must be divisible by 2 for bitstream packing
	res.size = res.data.len();
	if res.data.len() % 2 != 0 {
		res.data.push(0);
	}
	
	Ok(res)
}
fn decode_from_lzwstream(src: &LzwStream, mut callback: impl FnMut(f32)) -> Result<Vec<u8>, LzwError> {
	let mut res: Vec<u8> = vec![];
	
	// Initialize table with codes 0~255
	fn _create_table() -> CodeTableRev {
		Vec::from_iter((0..=255_u8).map(|c| vec![c]))
	}
	let mut dictionary = _create_table();
	
	fn find_code(dict: &CodeTableRev, code: SymbolUnit) -> Result<&[u8], LzwError> {
		let word = dict.get(code as usize)
			.ok_or(LzwError::ErrInvalidLzw)?;
		Ok(word)
	}
	
	let mut prev: Vec<u8> = vec![];
	let mut i = 0;
	
	for &b in src.data.iter() {
		if dictionary.len() == LZW_SIZE {
			// Reset dictionary
			dictionary = _create_table();
		}
		
		// No, I have no idea how this works
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
			callback(res.len() as f32 / src.data.len() as f32);
			i = 0;
		}
	}
	
	Ok(res)
}

fn lzwstream_to_bitstream(bitstream: &LzwStream) -> Result<Vec<u8>, LzwError> {
	if bitstream.data.len() % 2 != 0 {
		return Err(LzwError::ErrCompress(String::from("Invalid data size")));
	}
	
	// Initialize with header
	let mut res: Vec<u8> = vec![b'L', b'Z', b'W', b'S'];
	
	// Add data size
	res.extend_from_slice((bitstream.size as u32).to_le_bytes().as_slice());
	
	if BITSTREAM_PACK_SIMPLE {
		for i in bitstream.data.chunks(2) {
			res.push((i[0] & 0xff) as u8);
			res.push(((i[0] >> 8) & 0xff) as u8);
			res.push((i[1] & 0xff) as u8);
			res.push(((i[1] >> 8) & 0xff) as u8);
		}
	}
	else {
		// LzwStream unit = 12 bits
		// 2 units = 24 bits -> pack into 3 bytes
		for i in bitstream.data.chunks(2) {
			let i0h = ((i[0] >> 8) & 0xff) as u8;
			let i1h = ((i[1] >> 8) & 0xff) as u8;
			res.push((i[0] & 0xff) as u8);
			res.push((i[1] & 0xff) as u8);
			res.push((i1h << 4) | i0h);
		}
	}
	
	Ok(res)
}
fn bitstream_to_lzwstream(data: &[u8]) -> Result<LzwStream, LzwError> {
	// Verify header
	let len = data.len();
	if len < 8 
		|| (data[0] != b'L' || data[1] != b'Z'
			|| data[2] != b'W' || data[3] != b'S') {
		return Err(LzwError::ErrInvalidLzw);
	}
	
	let mut res: LzwStream = LzwStream {
		data: vec![],
		size: 0,
	};
	
	{
		let bytes_size = data.iter()
			.skip(4).take(4).cloned()
			.collect::<Vec<_>>();
		res.size = u32::from_le_bytes(bytes_size.try_into().unwrap()) as usize;
		
		// Verify data length
		let bitstream_stride = if BITSTREAM_PACK_SIMPLE { 4 } else { 3 };
		if (len - 8) % bitstream_stride != 0 {
			return Err(LzwError::ErrDecompress(String::from("Invalid bitstream size")));
		}
	}
	
	let skipped = data.iter()
		.skip(8)
		.collect::<Vec<_>>();
	
	// Unpack bitstream
	if BITSTREAM_PACK_SIMPLE {
		for i in skipped.chunks(4) {
			res.data.push((*i[0] as u16) | ((*i[1] as u16) << 8));
			res.data.push((*i[2] as u16) | ((*i[3] as u16) << 8));
		}
	}
	else {
		for i in skipped.chunks(3) {
			let i0h = (i[2] & 0xf) as u16;
			let i1h = ((i[2] >> 4) & 0xf) as u16;
			res.data.push((*i[0] as u16) | (i0h << 8));
			res.data.push((*i[1] as u16) | (i1h << 8));
		}
	}
	
	if res.data.len() < res.size {
		return Err(LzwError::ErrDecompress(String::from("Invalid bitstream")));
	}
	else {
		// Trim off excess data
		while res.data.len() > res.size {
			res.data.pop();
		}
	}
	
	Ok(res)
}
